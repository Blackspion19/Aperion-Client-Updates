use super::repository::{pack_dir, unknown_pack_error};
use super::{ResourcePackMetadata, get_resourcepack};
use crate::state::State;
use crate::util::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const RESOURCEPACKS_FOLDER_NAME: &str = "resourcepacks";
const SHORT_ID_LENGTH: usize = 8;
const MAX_SLUG_LENGTH: usize = 40;

/// First characters of the pack id, used to find a pack's deployed file again
/// even after the pack was renamed.
fn short_id(pack_id: &str) -> String {
    pack_id.chars().take(SHORT_ID_LENGTH).collect()
}

/// A readable file name part made from the pack name, e.g. `My Pack!` ->
/// `my-pack`. Minecraft shows the file name in its pack list, so a readable
/// name is much nicer than a raw UUID.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = true;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
        if slug.len() >= MAX_SLUG_LENGTH {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "pack".to_string()
    } else {
        slug
    }
}

/// File name a pack is deployed under: `<name>-<short id>.zip`.
pub(super) fn deployed_file_name(pack: &ResourcePackMetadata) -> String {
    format!("{}-{}.zip", slug(&pack.name), short_id(&pack.id))
}

/// All files in `resourcepacks_dir` that belong to the pack: the current
/// naming scheme, a previous name after a rename, and the legacy
/// `<full id>.zip` name that older versions used.
fn find_deployed_files(resourcepacks_dir: &Path, pack_id: &str) -> Vec<PathBuf> {
    let legacy_name = format!("{pack_id}.zip");
    let suffix = format!("-{}.zip", short_id(pack_id));

    let Ok(read_dir) = std::fs::read_dir(resourcepacks_dir) else {
        return Vec::new();
    };

    read_dir
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let file_name = entry.file_name().to_string_lossy().to_string();
            file_name == legacy_name || file_name.ends_with(&suffix)
        })
        .map(|entry| entry.path())
        .collect()
}

/// Build (or refresh) a `.zip` for the given pack inside the instance's
/// `resourcepacks/` folder, so Minecraft picks it up the next time the
/// resource pack list is refreshed. Replaces any previous deployment of the
/// same pack to that instance (also under an older file name).
pub async fn deploy_resourcepack_to_instance(
    pack_id: &str,
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let source_dir = pack_dir(&state, pack_id);
    if !source_dir.exists() {
        return Err(unknown_pack_error());
    }
    let metadata =
        get_resourcepack(pack_id).await?.ok_or_else(unknown_pack_error)?;

    let instance_dir = crate::instance::get_full_path(instance_id).await?;
    let resourcepacks_dir = instance_dir.join(RESOURCEPACKS_FOLDER_NAME);
    io::create_dir_all(&resourcepacks_dir).await?;

    let destination = resourcepacks_dir.join(deployed_file_name(&metadata));
    let previous = find_deployed_files(&resourcepacks_dir, pack_id);

    // Write to a temporary file first so a failed export never leaves a
    // half-written pack behind that Minecraft would then fail to load.
    let temporary = resourcepacks_dir.join(format!(
        ".{}.tmp",
        destination
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default()
    ));
    let temporary_clone = temporary.clone();

    let packaged = tokio::task::spawn_blocking(move || {
        zip_pack_folder(&source_dir, &temporary_clone)
    })
    .await
    .map_err(|_| {
        crate::Error::from(crate::ErrorKind::InputError(
            "Failed to package resource pack".to_string(),
        ))
    })?;

    if let Err(error) = packaged {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    for old in previous {
        if old != destination {
            let _ = std::fs::remove_file(&old);
        }
    }
    io::rename_or_move(&temporary, &destination).await?;

    Ok(())
}

/// Remove a previously deployed pack from an instance's `resourcepacks/`
/// folder. Does not delete the pack itself.
pub async fn remove_resourcepack_from_instance(
    pack_id: &str,
    instance_id: &str,
) -> crate::Result<()> {
    let instance_dir = crate::instance::get_full_path(instance_id).await?;
    let resourcepacks_dir = instance_dir.join(RESOURCEPACKS_FOLDER_NAME);

    for file in find_deployed_files(&resourcepacks_dir, pack_id) {
        io::remove_file(&file).await?;
    }

    Ok(())
}

/// Whether the given pack is currently deployed (as a `.zip`) into the
/// given instance's `resourcepacks/` folder.
pub async fn is_resourcepack_deployed_to_instance(
    pack_id: &str,
    instance_id: &str,
) -> crate::Result<bool> {
    let instance_dir = crate::instance::get_full_path(instance_id).await?;
    let resourcepacks_dir = instance_dir.join(RESOURCEPACKS_FOLDER_NAME);
    Ok(!find_deployed_files(&resourcepacks_dir, pack_id).is_empty())
}

/// Package a pack folder into a `.zip` file at `destination`.
pub(super) fn zip_pack_folder(
    source_dir: &Path,
    destination: &Path,
) -> crate::Result<()> {
    let file = std::fs::File::create(destination)
        .map_err(|error| io::IOError::with_path(error, destination))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);

    let mut entries = Vec::new();
    collect_entries(source_dir, source_dir, &mut entries)?;
    // Stable order: `pack.mcmeta` first (as most tools do), then the rest.
    entries.sort_by(|a, b| {
        let a_first = a.0 != "pack.mcmeta";
        let b_first = b.0 != "pack.mcmeta";
        a_first.cmp(&b_first).then_with(|| a.0.cmp(&b.0))
    });

    for (relative_path, absolute_path) in entries {
        writer
            .start_file(relative_path, options)
            .map_err(|error| input_error(error.to_string()))?;
        let bytes = std::fs::read(&absolute_path)
            .map_err(|error| io::IOError::with_path(error, &absolute_path))?;
        writer
            .write_all(&bytes)
            .map_err(|error| input_error(error.to_string()))?;
    }

    writer
        .finish()
        .map_err(|error| input_error(error.to_string()))?;
    Ok(())
}

/// Recursively list all files below `dir` as `(relative path with forward
/// slashes, absolute path)`.
pub(super) fn collect_entries(
    root: &Path,
    dir: &Path,
    entries: &mut Vec<(String, PathBuf)>,
) -> crate::Result<()> {
    let read_dir = std::fs::read_dir(dir)
        .map_err(|error| io::IOError::with_path(error, dir))?;
    for entry in read_dir {
        let entry =
            entry.map_err(|error| io::IOError::with_path(error, dir))?;
        let path = entry.path();
        if path.is_dir() {
            collect_entries(root, &path, entries)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            entries.push((relative, path));
        }
    }
    Ok(())
}

fn input_error(message: String) -> crate::Error {
    crate::ErrorKind::InputError(message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_readable_and_safe() {
        assert_eq!(slug("My Pack!"), "my-pack");
        assert_eq!(slug("  --Hello   World--  "), "hello-world");
        assert_eq!(slug("日本語"), "pack");
        assert_eq!(slug(""), "pack");
        assert!(slug(&"a".repeat(200)).len() <= MAX_SLUG_LENGTH);
        assert!(!slug("../../etc/passwd").contains('/'));
        assert!(!slug("../../etc/passwd").contains('.'));
    }

    #[test]
    fn short_id_is_a_prefix() {
        assert_eq!(short_id("12345678-aaaa-bbbb-cccc-dddddddddddd"), "12345678");
        assert_eq!(short_id("abc"), "abc");
    }
}
