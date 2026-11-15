//! Schematic manager (Litematica). Everything here works on the
//! `schematics` folder of a single instance, and every path the frontend
//! sends is relative to that folder and checked so it can never leave it.

use crate::api::Result;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::{Runtime, plugin::TauriPlugin};

const MAX_READ_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FOLDER_DEPTH: usize = 8;
const SCHEMATIC_EXTENSIONS: [&str; 4] = ["litematic", "schematic", "schem", "nbt"];

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("schematics")
        .invoke_handler(tauri::generate_handler![
            schematics_has_litematica,
            schematics_list_folders,
            schematics_list_files,
            schematics_read,
            schematics_import,
            schematics_create_folder,
            schematics_delete,
        ])
        .build()
}

#[derive(Serialize)]
pub struct SchematicFile {
    pub name: String,
    /// Path relative to the schematics folder, with `/` separators
    pub path: String,
    pub size: u64,
    /// Last modified, milliseconds since the Unix epoch
    pub modified: i64,
}

fn input_error(message: &str) -> crate::api::TheseusSerializableError {
    theseus::Error::from(theseus::ErrorKind::InputError(message.to_string())).into()
}

fn schematics_root(instance_dir: &Path) -> PathBuf {
    instance_dir.join("schematics")
}

/// Turn a relative path from the frontend into a path inside `root`.
/// Absolute paths, `..` and drive prefixes are rejected.
fn resolve(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            _ => return Err(input_error("Invalid path")),
        }
    }
    Ok(path)
}

/// Make sure a path (or its closest existing parent) really lies inside
/// `root`, even when symbolic links are involved.
fn ensure_inside(root: &Path, path: &Path) -> Result<()> {
    let Ok(root) = std::fs::canonicalize(root) else {
        // No schematics folder yet, so nothing can be outside of it.
        return Ok(());
    };
    let mut probe = path;
    while !probe.exists() {
        probe = probe
            .parent()
            .ok_or_else(|| input_error("Invalid path"))?;
    }
    let canonical = std::fs::canonicalize(probe)?;
    if canonical.starts_with(&root) {
        Ok(())
    } else {
        Err(input_error("Path is outside of the schematics folder"))
    }
}

fn has_schematic_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SCHEMATIC_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// True when the instance has a Litematica mod file in its `mods` folder.
#[tauri::command]
pub async fn schematics_has_litematica(instance_dir: PathBuf) -> Result<bool> {
    let Ok(entries) = std::fs::read_dir(instance_dir.join("mods")) else {
        return Ok(false);
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("litematica") && name.ends_with(".jar") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collect_folders(
    root: &Path,
    directory: &Path,
    depth: usize,
    output: &mut Vec<String>,
) {
    if depth > MAX_FOLDER_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Symbolic links are not followed, so a link cannot lead out of the folder.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            output.push(relative_string(root, &path));
            collect_folders(root, &path, depth + 1, output);
        }
    }
}

/// All sub folders of the schematics folder, as relative paths.
#[tauri::command]
pub async fn schematics_list_folders(instance_dir: PathBuf) -> Result<Vec<String>> {
    let root = schematics_root(&instance_dir);
    let mut folders = Vec::new();
    collect_folders(&root, &root, 0, &mut folders);
    folders.sort_by_key(|folder| folder.to_lowercase());
    Ok(folders)
}

/// The schematic files directly inside `folder` (empty string = top level).
#[tauri::command]
pub async fn schematics_list_files(
    instance_dir: PathBuf,
    folder: String,
) -> Result<Vec<SchematicFile>> {
    let root = schematics_root(&instance_dir);
    let directory = resolve(&root, &folder)?;
    ensure_inside(&root, &directory)?;

    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Ok(Vec::new());
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || !has_schematic_extension(&path) {
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        files.push(SchematicFile {
            name: entry.file_name().to_string_lossy().to_string(),
            path: relative_string(&root, &path),
            size: metadata.len(),
            modified,
        });
    }
    files.sort_by_key(|file| file.name.to_lowercase());
    Ok(files)
}

/// Raw bytes of one schematic file (sent as binary, not as a number list).
#[tauri::command]
pub async fn schematics_read(
    instance_dir: PathBuf,
    path: String,
) -> Result<tauri::ipc::Response> {
    let root = schematics_root(&instance_dir);
    let file = resolve(&root, &path)?;
    ensure_inside(&root, &file)?;
    if !has_schematic_extension(&file) {
        return Err(input_error("Not a schematic file"));
    }
    let size = std::fs::metadata(&file)?.len();
    if size > MAX_READ_BYTES {
        return Err(input_error("The schematic is too large to open"));
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(file))
        .await
        .map_err(|_| input_error("Reading the file failed"))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Pick a name that does not exist yet: `name.litematic`, `name (2).litematic`, ...
fn unique_target(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "schematic".to_string());
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    for number in 2..10_000 {
        let candidate = directory.join(format!("{stem} ({number}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(file_name)
}

/// Copy files (for example dropped onto the page) into `folder`.
/// Returns the relative paths of the new files; unsupported files are skipped.
#[tauri::command]
pub async fn schematics_import(
    instance_dir: PathBuf,
    folder: String,
    sources: Vec<PathBuf>,
) -> Result<Vec<String>> {
    let root = schematics_root(&instance_dir);
    let directory = resolve(&root, &folder)?;
    ensure_inside(&root, &directory)?;
    std::fs::create_dir_all(&directory)?;

    let mut imported = Vec::new();
    for source in sources {
        if !source.is_file() || !has_schematic_extension(&source) {
            continue;
        }
        let Some(file_name) = source.file_name().map(|name| name.to_string_lossy().to_string())
        else {
            continue;
        };
        let target = unique_target(&directory, &file_name);
        std::fs::copy(&source, &target)?;
        imported.push(relative_string(&root, &target));
    }
    Ok(imported)
}

/// Create a (possibly nested) folder below the schematics folder.
#[tauri::command]
pub async fn schematics_create_folder(
    instance_dir: PathBuf,
    path: String,
) -> Result<()> {
    let root = schematics_root(&instance_dir);
    let folder = resolve(&root, &path)?;
    if folder == root {
        return Err(input_error("Invalid folder name"));
    }
    ensure_inside(&root, &folder)?;
    std::fs::create_dir_all(&folder)?;
    Ok(())
}

/// Delete one schematic file or a whole folder (never the top level folder).
#[tauri::command]
pub async fn schematics_delete(instance_dir: PathBuf, path: String) -> Result<()> {
    let root = schematics_root(&instance_dir);
    let target = resolve(&root, &path)?;
    if target == root {
        return Err(input_error("The top level folder cannot be deleted"));
    }
    ensure_inside(&root, &target)?;
    if target.is_dir() {
        std::fs::remove_dir_all(&target)?;
    } else if target.is_file() && has_schematic_extension(&target) {
        std::fs::remove_file(&target)?;
    }
    Ok(())
}
