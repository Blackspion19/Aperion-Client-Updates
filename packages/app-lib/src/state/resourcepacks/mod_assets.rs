//! Reads assets (blockstates, models, textures) out of the mod jars of an
//! instance, so previews can show blocks that come from mods.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;
use zip::ZipArchive;

const MAX_ASSET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_JAR_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PATHS: usize = 512;
const MISSING: u32 = u32::MAX;

/// What is known about one jar: which asset files it contains.
#[derive(Clone)]
struct JarIndex {
    len: u64,
    modified: Option<SystemTime>,
    namespaces: Vec<String>,
    entries: Arc<HashSet<String>>,
}

// Parsing the file table of a jar is the slow part, so it is done once per
// jar (until the file changes).
static JAR_INDEXES: LazyLock<Mutex<HashMap<PathBuf, JarIndex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn input_error(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.to_string()).into()
}

fn is_asset_entry(name: &str) -> bool {
    name.starts_with("assets/")
        && (name.contains("/textures/")
            || name.contains("/models/")
            || name.contains("/blockstates/"))
}

fn index_jar(path: &Path) -> Option<JarIndex> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_JAR_BYTES {
        return None;
    }
    let len = metadata.len();
    let modified = metadata.modified().ok();

    if let Ok(cache) = JAR_INDEXES.lock()
        && let Some(index) = cache.get(path)
        && index.len == len
        && index.modified == modified
    {
        return Some(index.clone());
    }

    let file = File::open(path).ok()?;
    let archive = ZipArchive::new(BufReader::new(file)).ok()?;

    let mut namespaces = BTreeSet::new();
    let mut entries = HashSet::new();
    for name in archive.file_names() {
        if !is_asset_entry(name) {
            continue;
        }
        if let Some((namespace, _)) =
            name.trim_start_matches("assets/").split_once('/')
        {
            namespaces.insert(namespace.to_string());
        }
        entries.insert(name.to_string());
    }

    let index = JarIndex {
        len,
        modified,
        namespaces: namespaces.into_iter().collect(),
        entries: Arc::new(entries),
    };
    if let Ok(mut cache) = JAR_INDEXES.lock() {
        cache.insert(path.to_path_buf(), index.clone());
    }
    Some(index)
}

fn list_jars(mods_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };

    let mut jars: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
        })
        .collect();
    jars.sort();
    jars
}

/// `assets/<namespace>/<rest>`; the namespace `*` stands for "any mod".
fn split_asset_path(path: &str) -> Option<(&str, &str)> {
    if path.contains("..") || path.contains('\\') || path.starts_with('/') {
        return None;
    }
    path.strip_prefix("assets/")?.split_once('/')
}

fn read_entry(
    archive: &mut ZipArchive<BufReader<File>>,
    name: &str,
) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    if entry.size() > MAX_ASSET_BYTES {
        return None;
    }
    let mut buffer = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

fn find_asset(
    path: &str,
    jars: &[(PathBuf, JarIndex)],
    open: &mut HashMap<usize, ZipArchive<BufReader<File>>>,
) -> Option<Vec<u8>> {
    let (namespace, rest) = split_asset_path(path)?;

    for (position, (jar_path, index)) in jars.iter().enumerate() {
        let candidates: Vec<String> = if namespace == "*" {
            index
                .namespaces
                .iter()
                .map(|namespace| format!("assets/{namespace}/{rest}"))
                .filter(|name| index.entries.contains(name))
                .collect()
        } else if index.entries.contains(path) {
            vec![path.to_string()]
        } else {
            Vec::new()
        };

        let Some(name) = candidates.first() else {
            continue;
        };

        if let std::collections::hash_map::Entry::Vacant(slot) =
            open.entry(position)
        {
            let Ok(file) = File::open(jar_path) else {
                continue;
            };
            let Ok(archive) = ZipArchive::new(BufReader::new(file)) else {
                continue;
            };
            slot.insert(archive);
        }

        if let Some(archive) = open.get_mut(&position)
            && let Some(bytes) = read_entry(archive, name)
        {
            return Some(bytes);
        }
    }

    None
}

fn read_mod_assets_blocking(mods_dir: &Path, paths: &[String]) -> Vec<u8> {
    let jars: Vec<(PathBuf, JarIndex)> = list_jars(mods_dir)
        .into_iter()
        .filter_map(|path| index_jar(&path).map(|index| (path, index)))
        .collect();

    let mut open = HashMap::new();
    let mut output = Vec::new();

    for path in paths {
        match find_asset(path, &jars, &mut open) {
            Some(bytes) => {
                output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                output.extend_from_slice(&bytes);
            }
            None => output.extend_from_slice(&MISSING.to_le_bytes()),
        }
    }

    output
}

/// Reads several assets from the mod jars of an instance in one go.
///
/// Every path looks like `assets/<namespace>/<path>`; with `*` as namespace
/// all mods are searched. The result holds one block per requested path, in
/// order: a little-endian `u32` length followed by the bytes, or `u32::MAX`
/// when the asset does not exist.
pub async fn read_mod_assets(
    instance_id: &str,
    paths: Vec<String>,
) -> crate::Result<Vec<u8>> {
    if paths.len() > MAX_PATHS {
        return Err(input_error("Too many assets requested"));
    }

    let mods_dir = crate::api::instance::get_full_path(instance_id)
        .await?
        .join("mods");

    tokio::task::spawn_blocking(move || {
        read_mod_assets_blocking(&mods_dir, &paths)
    })
    .await
    .map_err(|_| input_error("Failed to read mod assets"))
}
