use crate::state::State;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const MAX_ARCHIVE_ENTRIES: usize = 200_000;
const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;
const SOUND_PREFIX: &str = "assets/minecraft/sounds/";

/// A single overridable vanilla asset discovered for a game version (a
/// texture, sound, font, etc. under `assets/`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VanillaAsset {
    /// Path relative to the pack root, e.g.
    /// `assets/minecraft/textures/block/stone.png`.
    pub path: String,
    pub kind: VanillaAssetKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VanillaAssetKind {
    Texture,
    Sound,
    Font,
    Other,
}

impl VanillaAssetKind {
    fn from_path(path: &str) -> Self {
        if path.starts_with("assets/minecraft/textures/")
            && path.ends_with(".png")
        {
            Self::Texture
        } else if path.starts_with(SOUND_PREFIX) && path.ends_with(".ogg") {
            Self::Sound
        } else if path.starts_with("assets/minecraft/font/") {
            Self::Font
        } else {
            Self::Other
        }
    }
}

fn client_jar_path(state: &State, game_version: &str) -> PathBuf {
    state
        .directories
        .version_dir(game_version)
        .join(format!("{game_version}.jar"))
}

/// List every overridable asset (textures, sounds, fonts) of the given game
/// version. Textures and fonts come from the client jar; sounds are not part
/// of the jar and are looked up through the game's asset index instead.
/// A missing client jar, version file or asset index is downloaded first
/// (into the folders shared by all instances and packs).
pub async fn list_vanilla_assets(
    game_version: &str,
) -> crate::Result<Vec<VanillaAsset>> {
    let state = State::get().await?;
    let jar_path = client_jar_path(&state, game_version);

    ensure_version_downloaded(&state, game_version).await?;

    let mut assets = run_blocking(move || parse_jar_assets(&jar_path)).await?;

    match load_sound_assets(&state, game_version).await {
        Ok(sounds) => assets.extend(sounds),
        Err(error) => {
            tracing::warn!(
                "Could not read the sound list for Minecraft {game_version}: {error}"
            );
        }
    }

    assets.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(assets)
}

/// Read a single asset's raw bytes, so the editor can show/play the vanilla
/// texture or sound the user is about to override.
pub async fn read_vanilla_asset(
    game_version: &str,
    relative_asset_path: &str,
) -> crate::Result<Vec<u8>> {
    let state = State::get().await?;

    if relative_asset_path.starts_with(SOUND_PREFIX)
        && relative_asset_path.ends_with(".ogg")
    {
        return read_vanilla_sound(&state, game_version, relative_asset_path)
            .await;
    }

    ensure_version_downloaded(&state, game_version).await?;
    let jar_path = client_jar_path(&state, game_version);
    let relative_asset_path = relative_asset_path.to_string();

    run_blocking(move || read_jar_entry(&jar_path, &relative_asset_path)).await
}

/// The most recently used client jar stays open. Opening a 30+ MB jar means
/// parsing its whole file table, which used to happen for every single
/// thumbnail and made the texture list lag.
static JAR_CACHE: std::sync::Mutex<
    Option<(PathBuf, ZipArchive<BufReader<File>>)>,
> = std::sync::Mutex::new(None);

fn read_jar_entry(jar_path: &Path, name: &str) -> crate::Result<Vec<u8>> {
    let mut cache = JAR_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let is_cached = matches!(&*cache, Some((path, _)) if path == jar_path);
    if !is_cached {
        *cache = Some((jar_path.to_path_buf(), open_archive(jar_path)?));
    }
    let Some((_, archive)) = cache.as_mut() else {
        return Err(input_error("Unknown vanilla asset".to_string()));
    };

    let mut entry = archive
        .by_name(name)
        .map_err(|_| input_error("Unknown vanilla asset".to_string()))?;
    if entry.size() > MAX_ASSET_BYTES {
        return Err(input_error("Asset is too large to preview".to_string()));
    }
    let mut buffer = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut buffer)
        .map_err(|error| input_error(error.to_string()))?;
    Ok(buffer)
}

/// Make sure the client jar, the version file and the asset index of a
/// version exist locally. They live in the launcher's shared `versions` and
/// `assets` folders, so every pack and instance of a version reuses them and
/// nothing is downloaded twice. When everything is present no network
/// access happens at all.
async fn ensure_version_downloaded(
    state: &State,
    game_version: &str,
) -> crate::Result<()> {
    // Versions checked once stay checked, so the thumbnails of the texture
    // list do not each re-read the asset index from disk.
    if ENSURED_VERSIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(game_version)
    {
        return Ok(());
    }

    let jar_path = client_jar_path(state, game_version);
    if jar_path.exists() && load_asset_index(state, game_version).await.is_ok()
    {
        mark_version_ensured(game_version);
        return Ok(());
    }

    let (minecraft, version_index) =
        crate::launcher::resolve_minecraft_manifest(game_version, state)
            .await?;
    let version = &minecraft.versions[version_index];
    let version_info = crate::launcher::download::download_version_info(
        state, version, None, None, None, None,
    )
    .await?;
    crate::launcher::download::download_assets_index(
        state,
        &version_info,
        None,
        false,
        None,
    )
    .await?;
    crate::launcher::download::download_client(
        state,
        &version_info,
        None,
        false,
        None,
    )
    .await?;
    mark_version_ensured(game_version);
    Ok(())
}

static ENSURED_VERSIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<String>>,
> = std::sync::LazyLock::new(Default::default);

fn mark_version_ensured(game_version: &str) {
    ENSURED_VERSIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(game_version.to_string());
}

async fn run_blocking<T, F>(task: F) -> crate::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> crate::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task).await.map_err(|_| {
        crate::Error::from(crate::ErrorKind::InputError(
            "Failed to read Minecraft assets".to_string(),
        ))
    })?
}

fn open_archive(path: &Path) -> crate::Result<ZipArchive<BufReader<File>>> {
    // Reading through the file instead of loading the whole (30+ MB) jar
    // into memory keeps previews of many textures fast.
    let file = File::open(path)
        .map_err(|error| crate::util::io::IOError::with_path(error, path))?;
    ZipArchive::new(BufReader::new(file))
        .map_err(|error| input_error(error.to_string()))
}

fn parse_jar_assets(jar_path: &Path) -> crate::Result<Vec<VanillaAsset>> {
    let mut archive = open_archive(jar_path)?;

    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(input_error(
            "Minecraft client jar has too many entries".to_string(),
        ));
    }

    let mut assets = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| input_error(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.name();
        if !path.starts_with("assets/") {
            continue;
        }

        let kind = VanillaAssetKind::from_path(path);
        if kind == VanillaAssetKind::Other || kind == VanillaAssetKind::Sound {
            continue;
        }

        assets.push(VanillaAsset {
            path: path.to_string(),
            kind,
        });
    }

    Ok(assets)
}

/// Parsed contents of the asset index that belongs to a game version.
async fn load_asset_index(
    state: &State,
    game_version: &str,
) -> crate::Result<serde_json::Value> {
    let version_json_path = state
        .directories
        .version_dir(game_version)
        .join(format!("{game_version}.json"));
    let version_json: serde_json::Value = serde_json::from_slice(
        &crate::util::io::read(&version_json_path).await?,
    )?;
    let index_id = version_json
        .get("assetIndex")
        .and_then(|index| index.get("id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| {
            input_error("The version has no asset index".to_string())
        })?;

    let index_path = state
        .directories
        .assets_index_dir()
        .join(format!("{index_id}.json"));
    Ok(serde_json::from_slice(
        &crate::util::io::read(&index_path).await?,
    )?)
}

async fn load_sound_assets(
    state: &State,
    game_version: &str,
) -> crate::Result<Vec<VanillaAsset>> {
    let index = load_asset_index(state, game_version).await?;
    let objects = index
        .get("objects")
        .and_then(|objects| objects.as_object())
        .ok_or_else(|| input_error("The asset index is invalid".to_string()))?;

    Ok(objects
        .keys()
        .filter(|key| {
            key.starts_with("minecraft/sounds/") && key.ends_with(".ogg")
        })
        .map(|key| VanillaAsset {
            path: format!("assets/{key}"),
            kind: VanillaAssetKind::Sound,
        })
        .collect())
}

async fn read_vanilla_sound(
    state: &State,
    game_version: &str,
    relative_asset_path: &str,
) -> crate::Result<Vec<u8>> {
    let key = relative_asset_path
        .strip_prefix("assets/")
        .ok_or_else(|| input_error("Unknown vanilla asset".to_string()))?;

    let index = load_asset_index(state, game_version).await?;
    let object = index
        .get("objects")
        .and_then(|objects| objects.get(key))
        .ok_or_else(|| input_error("Unknown vanilla asset".to_string()))?;
    let hash = object
        .get("hash")
        .and_then(|hash| hash.as_str())
        .ok_or_else(|| input_error("Unknown vanilla asset".to_string()))?;
    let size = object.get("size").and_then(|size| size.as_u64()).unwrap_or(0);

    // The hash is used to build a file path, so only accept what a real
    // asset index contains.
    if hash.len() < 2 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(input_error("The asset index is invalid".to_string()));
    }

    // Sounds are not part of the jar. Fetch just this one when it is missing.
    crate::launcher::download::download_asset_object(state, key, hash, size)
        .await?;

    let object_path = state.directories.object_dir(hash);
    let metadata = crate::util::io::metadata(&object_path).await?;
    if metadata.len() > MAX_ASSET_BYTES {
        return Err(input_error("Asset is too large to preview".to_string()));
    }
    Ok(crate::util::io::read(&object_path).await?)
}

fn input_error(message: String) -> crate::Error {
    crate::ErrorKind::InputError(message).into()
}