//! Export and import of resource packs as `.zip` files.
//!
//! * Export zips the pack folder to a path chosen by the user. The zip is
//!   written to a temporary file first so a failed export never leaves a
//!   broken file behind.
//! * Import reads an existing resource pack `.zip`, copies only `assets/`
//!   and `pack.png` into a fresh pack and writes a new `pack.mcmeta` for
//!   the chosen game version. Extraction is zip-slip safe and size limited.

use super::deploy::zip_pack_folder;
use super::repository::{
    MAX_NAME_LENGTH, pack_dir, touch_pack_assets, unknown_pack_error,
};
use super::{
    ResourcePackMetadata, create_resourcepack, delete_resourcepack,
    edit_resourcepack,
};
use crate::state::State;
use crate::util::io;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const MAX_ENTRIES: usize = 100_000;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MCMETA_BYTES: u64 = 1024 * 1024;

fn input_error(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

async fn run_blocking<T, F>(task: F) -> crate::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> crate::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|_| input_error("The background task failed"))?
}

/// Export a pack as a `.zip` to `destination`. A missing `.zip` extension is
/// added. Returns the path that was written.
pub async fn export_resourcepack(
    pack_id: &str,
    destination: PathBuf,
) -> crate::Result<PathBuf> {
    let state = State::get().await?;
    let source_dir = pack_dir(&state, pack_id);
    if !source_dir.exists() {
        return Err(unknown_pack_error());
    }

    let mut destination = destination;
    let has_zip_extension = destination
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
    if !has_zip_extension {
        let mut name = destination.as_os_str().to_os_string();
        name.push(".zip");
        destination = PathBuf::from(name);
    }

    let file_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| input_error("Invalid export path"))?;
    let temporary = destination.with_file_name(format!(".{file_name}.tmp"));

    let temporary_clone = temporary.clone();
    let packaged =
        run_blocking(move || zip_pack_folder(&source_dir, &temporary_clone))
            .await;
    if let Err(error) = packaged {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    if destination.exists() {
        io::remove_file(&destination).await?;
    }
    io::rename_or_move(&temporary, &destination).await?;
    Ok(destination)
}

struct ExtractedPack {
    assets: Vec<String>,
    has_icon: bool,
}

/// Packs downloaded from the web are often zipped with a folder around them
/// (`MyPack/assets/...`). This finds that folder: the shortest path in front
/// of an `assets` folder or a `pack.mcmeta`. Returns the parts of the path
/// (empty when the pack starts at the top of the archive).
fn find_root_prefix(archive: &ZipArchive<BufReader<File>>) -> Vec<String> {
    let mut best: Option<Vec<String>> = None;
    for name in archive.file_names() {
        let normalized = name.replace('\\', "/");
        let parts: Vec<&str> =
            normalized.split('/').filter(|part| !part.is_empty()).collect();
        for (index, part) in parts.iter().enumerate() {
            let is_marker = (*part == "assets" && index + 1 < parts.len())
                || (*part == "pack.mcmeta" && index + 1 == parts.len());
            if is_marker {
                if index <= 3 {
                    let prefix: Vec<String> =
                        parts[..index].iter().map(|p| p.to_string()).collect();
                    if best.as_ref().is_none_or(|current| prefix.len() < current.len()) {
                        best = Some(prefix);
                    }
                }
                break;
            }
        }
    }
    best.unwrap_or_default()
}

/// Read the description out of a `pack.mcmeta`. It is either a plain string
/// or a text component (`{"text": "..."}`); anything else gives an empty
/// description.
fn read_description(
    archive: &mut ZipArchive<BufReader<File>>,
    prefix: &[String],
) -> String {
    let meta_name = if prefix.is_empty() {
        "pack.mcmeta".to_string()
    } else {
        format!("{}/pack.mcmeta", prefix.join("/"))
    };
    let Ok(entry) = archive.by_name(&meta_name) else {
        return String::new();
    };
    let mut bytes = Vec::new();
    if entry.take(MAX_MCMETA_BYTES).read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    // Some tools write a UTF-8 BOM, which serde_json rejects.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return String::new();
    };
    let description = value
        .get("pack")
        .and_then(|pack| pack.get("description"))
        .cloned()
        .unwrap_or_default();

    let text = match &description {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Object(object) => object
            .get("text")
            .and_then(|text| text.as_str())
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    };
    // Remove formatting codes such as `§a`.
    let mut cleaned = String::new();
    let mut skip_next = false;
    for character in text.chars() {
        if skip_next {
            skip_next = false;
        } else if character == '§' {
            skip_next = true;
        } else {
            cleaned.push(character);
        }
    }
    cleaned.trim().chars().take(256).collect()
}

fn open_zip(path: &Path) -> crate::Result<ZipArchive<BufReader<File>>> {
    let file = File::open(path)
        .map_err(|error| io::IOError::with_path(error, path))?;
    ZipArchive::new(BufReader::new(file))
        .map_err(|_| input_error("This file is not a valid .zip archive"))
}

fn extract_pack(
    zip_path: &Path,
    target_dir: &Path,
) -> crate::Result<ExtractedPack> {
    let mut archive = open_zip(zip_path)?;
    let prefix = find_root_prefix(&archive);

    if archive.len() > MAX_ENTRIES {
        return Err(input_error("The resource pack has too many files"));
    }

    let mut assets = Vec::new();
    let mut has_icon = false;
    let mut total_bytes: u64 = 0;

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| input_error(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        // `enclosed_name` rejects absolute paths and `..` (zip-slip).
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let parts: Vec<String> = enclosed
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(part) => {
                    Some(part.to_string_lossy().to_string())
                }
                _ => None,
            })
            .collect();
        // Everything has to lie inside the folder that holds the pack.
        if parts.len() <= prefix.len() || parts[..prefix.len()] != prefix[..] {
            continue;
        }
        let relative = parts[prefix.len()..].join("/");

        let is_icon = relative == "pack.png";
        if !is_icon && !relative.starts_with("assets/") {
            continue;
        }
        if entry.size() > MAX_FILE_BYTES {
            return Err(input_error(format!(
                "The file {relative} in the resource pack is too large"
            )));
        }

        let destination = target_dir.join(&relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| io::IOError::with_path(error, parent))?;
        }

        // The declared size in a zip can be wrong, so the real number of
        // bytes is limited while copying.
        let mut output = File::create(&destination)
            .map_err(|error| io::IOError::with_path(error, &destination))?;
        let mut limited = entry.take(MAX_FILE_BYTES + 1);
        let copied = std::io::copy(&mut limited, &mut output)
            .map_err(|error| input_error(error.to_string()))?;
        output
            .flush()
            .map_err(|error| io::IOError::with_path(error, &destination))?;
        if copied > MAX_FILE_BYTES {
            return Err(input_error(format!(
                "The file {relative} in the resource pack is too large"
            )));
        }

        total_bytes += copied;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(input_error("The resource pack is too large"));
        }

        if is_icon {
            has_icon = true;
        } else {
            assets.push(relative);
        }
    }

    if assets.is_empty() && !has_icon {
        return Err(input_error(
            "This .zip contains no resource pack (no assets folder found)",
        ));
    }

    assets.sort();
    Ok(ExtractedPack { assets, has_icon })
}

/// Import a resource pack `.zip` as a new pack for `game_version`. When
/// `name` is `None` the file name is used.
pub async fn import_resourcepack(
    zip_path: PathBuf,
    name: Option<String>,
    game_version: String,
    max_game_version: Option<String>,
) -> crate::Result<ResourcePackMetadata> {
    if !zip_path.is_file() {
        return Err(input_error("The selected file does not exist"));
    }

    let name = name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| {
            zip_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().trim().to_string())
                .filter(|stem| !stem.is_empty())
        })
        .unwrap_or_else(|| "Imported pack".to_string());
    let name: String = name.chars().take(MAX_NAME_LENGTH).collect();

    let state = State::get().await?;

    // Read the description first so the new pack's `pack.mcmeta` carries it.
    let zip_for_description = zip_path.clone();
    let description = run_blocking(move || {
        let mut archive = open_zip(&zip_for_description)?;
        let prefix = find_root_prefix(&archive);
        Ok(read_description(&mut archive, &prefix))
    })
    .await?;

    let metadata =
        create_resourcepack(name, description, game_version, max_game_version)
            .await?;
    let target_dir = pack_dir(&state, &metadata.id);

    let extraction = {
        let target_dir = target_dir.clone();
        run_blocking(move || extract_pack(&zip_path, &target_dir)).await
    };
    let extracted = match extraction {
        Ok(extracted) => extracted,
        Err(error) => {
            let _ = delete_resourcepack(&metadata.id).await;
            return Err(error);
        }
    };

    if extracted.has_icon {
        let icon = target_dir.join("pack.png").to_string_lossy().to_string();
        edit_resourcepack(&metadata.id, None, None, Some(Some(icon))).await?;
    }

    touch_pack_assets(&state, &metadata.id, extracted.assets).await
}
