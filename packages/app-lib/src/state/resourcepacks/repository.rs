use super::PackFormat;
use crate::state::State;
use crate::util::io;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const INDEX_FILE_NAME: &str = "index.json";
pub(super) const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 256;

/// Metadata about a single user-created resource pack. Stored in the shared
/// index file at `resourcepacks_dir()/index.json`; the pack's actual content
/// (a loose folder of assets) lives at `resourcepacks_dir()/<id>/`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourcePackMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pack_format: u32,
    /// The Minecraft version this pack was created for; kept only for
    /// display purposes (choosing a `pack_format` again if the user wants to
    /// retarget the pack).
    pub game_version: String,
    /// The newest Minecraft version the pack is meant for. `None` means only
    /// `game_version`; `"*"` means no upper limit (this version and newer).
    #[serde(default)]
    pub max_game_version: Option<String>,
    pub icon_path: Option<String>,
    pub created: chrono::DateTime<chrono::Utc>,
    pub modified: chrono::DateTime<chrono::Utc>,
    /// Relative asset paths (e.g. `assets/minecraft/textures/block/stone.png`)
    /// that have been overridden by the user, for display in the editor.
    #[serde(default)]
    pub overridden_assets: Vec<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct ResourcePackIndex {
    #[serde(default)]
    pub(super) packs: Vec<ResourcePackMetadata>,
}

fn validate_name(name: &str) -> crate::Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_LENGTH {
        return Err(crate::ErrorKind::InputError(format!(
            "Resource pack name must be between 1 and {MAX_NAME_LENGTH} characters"
        ))
        .into());
    }
    Ok(())
}

fn validate_description(description: &str) -> crate::Result<()> {
    if description.chars().count() > MAX_DESCRIPTION_LENGTH {
        return Err(crate::ErrorKind::InputError(format!(
            "Resource pack description must be at most {MAX_DESCRIPTION_LENGTH} characters"
        ))
        .into());
    }
    Ok(())
}

async fn index_path(state: &State) -> PathBuf {
    state.directories.resourcepacks_dir().join(INDEX_FILE_NAME)
}

pub(super) async fn read_index(state: &State) -> crate::Result<ResourcePackIndex> {
    let path = index_path(state).await;
    if !path.exists() {
        return Ok(ResourcePackIndex::default());
    }

    let bytes = io::read(&path).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) async fn write_index(
    state: &State,
    index: &ResourcePackIndex,
) -> crate::Result<()> {
    let dir = state.directories.resourcepacks_dir();
    io::create_dir_all(&dir).await?;
    let bytes = serde_json::to_vec_pretty(index)?;
    io::write(index_path(state).await, &bytes).await?;
    Ok(())
}

/// Directory that a pack's loose asset files live in.
pub fn pack_dir(state: &State, id: &str) -> PathBuf {
    state.directories.resourcepacks_dir().join(id)
}

/// List all resource packs known to the launcher, newest-modified first.
pub async fn list_resourcepacks() -> crate::Result<Vec<ResourcePackMetadata>> {
    let state = State::get().await?;
    let mut index = read_index(&state).await?;
    refresh_pack_formats(&state, &mut index).await?;
    index.packs.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(index.packs)
}

/// Get a single resource pack's metadata.
pub async fn get_resourcepack(
    id: &str,
) -> crate::Result<Option<ResourcePackMetadata>> {
    let state = State::get().await?;
    let mut index = read_index(&state).await?;
    refresh_pack_formats(&state, &mut index).await?;
    Ok(index.packs.into_iter().find(|pack| pack.id == id))
}

/// Packs created by older versions of the launcher may carry a wrong
/// `pack_format` (the old lookup table ended at 1.21.8 and mixed up data
/// pack and resource pack numbers). Recompute it from the pack's game
/// version and rewrite `pack.mcmeta` when it changed.
async fn refresh_pack_formats(
    state: &State,
    index: &mut ResourcePackIndex,
) -> crate::Result<()> {
    let mut changed = false;
    for pack in index.packs.iter_mut() {
        let expected = PackFormat::for_game_version(&pack.game_version).0;
        if pack.pack_format != expected {
            pack.pack_format = expected;
            super::writer::write_pack_mcmeta(state, pack).await?;
            changed = true;
        }
    }
    if changed {
        write_index(state, index).await?;
    }
    Ok(())
}

/// Create a new, empty resource pack (just a `pack.mcmeta`) targeting the
/// given game version. Returns the new pack's metadata.
pub async fn create_resourcepack(
    name: String,
    description: String,
    game_version: String,
    max_game_version: Option<String>,
) -> crate::Result<ResourcePackMetadata> {
    validate_name(&name)?;
    validate_description(&description)?;
    let max_game_version = normalize_max_version(max_game_version)?;

    let state = State::get().await?;
    let id = uuid::Uuid::new_v4().to_string();
    let pack_format = PackFormat::for_game_version(&game_version).0;
    let now = chrono::Utc::now();

    let metadata = ResourcePackMetadata {
        id: id.clone(),
        name: name.trim().to_string(),
        description: description.trim().to_string(),
        pack_format,
        game_version,
        max_game_version,
        icon_path: None,
        created: now,
        modified: now,
        overridden_assets: Vec::new(),
    };

    super::writer::init_pack_folder(&state, &metadata).await?;

    let mut index = read_index(&state).await?;
    index.packs.push(metadata.clone());
    write_index(&state, &index).await?;

    Ok(metadata)
}

/// Update a pack's display name/description/icon. Does not touch its assets.
pub async fn edit_resourcepack(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    icon_path: Option<Option<String>>,
) -> crate::Result<ResourcePackMetadata> {
    if let Some(name) = &name {
        validate_name(name)?;
    }
    if let Some(description) = &description {
        validate_description(description)?;
    }

    let state = State::get().await?;
    let mut index = read_index(&state).await?;
    let pack = index
        .packs
        .iter_mut()
        .find(|pack| pack.id == id)
        .ok_or_else(unknown_pack_error)?;

    if let Some(name) = name {
        pack.name = name.trim().to_string();
    }
    if let Some(description) = description {
        pack.description = description.trim().to_string();
    }
    if let Some(icon_path) = icon_path {
        pack.icon_path = icon_path;
    }
    pack.modified = chrono::Utc::now();

    let updated = pack.clone();
    super::writer::write_pack_mcmeta(&state, &updated).await?;
    write_index(&state, &index).await?;

    Ok(updated)
}

/// Record that an asset override was added/removed, updating `modified` and
/// the `overridden_assets` list. Used by the writer after it changes files
/// on disk so the index stays in sync.
pub(super) async fn touch_pack_assets(
    state: &State,
    id: &str,
    overridden_assets: Vec<String>,
) -> crate::Result<ResourcePackMetadata> {
    let mut index = read_index(state).await?;
    let pack = index
        .packs
        .iter_mut()
        .find(|pack| pack.id == id)
        .ok_or_else(unknown_pack_error)?;

    pack.overridden_assets = overridden_assets;
    pack.modified = chrono::Utc::now();

    let updated = pack.clone();
    write_index(state, &index).await?;
    Ok(updated)
}

/// Check the optional upper version of a pack's range. Empty means "only the
/// one version"; `"*"` means no upper limit.
fn normalize_max_version(
    max_game_version: Option<String>,
) -> crate::Result<Option<String>> {
    let Some(version) = max_game_version else {
        return Ok(None);
    };
    let version = version.trim().to_string();
    if version.is_empty() {
        return Ok(None);
    }
    if version.chars().count() > 32 || version.contains(['/', '\\']) {
        return Err(crate::ErrorKind::InputError(
            "Invalid game version".to_string(),
        )
        .into());
    }
    Ok(Some(version))
}

/// Retarget a pack to another range of game versions: updates its pack
/// format and rewrites `pack.mcmeta`. The assets themselves are not converted.
pub async fn retarget_resourcepack(
    id: &str,
    game_version: String,
    max_game_version: Option<String>,
) -> crate::Result<ResourcePackMetadata> {
    let max_game_version = normalize_max_version(max_game_version)?;
    let game_version = game_version.trim().to_string();
    if game_version.is_empty()
        || game_version.chars().count() > 32
        || game_version.contains(['/', '\\'])
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid game version".to_string(),
        )
        .into());
    }

    let state = State::get().await?;
    let mut index = read_index(&state).await?;
    let pack = index
        .packs
        .iter_mut()
        .find(|pack| pack.id == id)
        .ok_or_else(unknown_pack_error)?;

    pack.pack_format = PackFormat::for_game_version(&game_version).0;
    pack.game_version = game_version;
    pack.max_game_version = max_game_version;
    pack.modified = chrono::Utc::now();

    let updated = pack.clone();
    super::writer::write_pack_mcmeta(&state, &updated).await?;
    write_index(&state, &index).await?;

    Ok(updated)
}

/// Create an independent copy of a pack (all assets included).
pub async fn duplicate_resourcepack(
    id: &str,
) -> crate::Result<ResourcePackMetadata> {
    let state = State::get().await?;
    let mut index = read_index(&state).await?;
    let source = index
        .packs
        .iter()
        .find(|pack| pack.id == id)
        .cloned()
        .ok_or_else(unknown_pack_error)?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let source_dir = pack_dir(&state, id);
    let target_dir = pack_dir(&state, &new_id);

    {
        let source_dir = source_dir.clone();
        let target_dir = target_dir.clone();
        tokio::task::spawn_blocking(move || -> crate::Result<()> {
            let mut entries = Vec::new();
            if source_dir.exists() {
                super::deploy::collect_entries(
                    &source_dir,
                    &source_dir,
                    &mut entries,
                )?;
            }
            std::fs::create_dir_all(&target_dir)
                .map_err(|error| io::IOError::with_path(error, &target_dir))?;
            for (relative, absolute) in entries {
                let destination = target_dir.join(&relative);
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        io::IOError::with_path(error, parent)
                    })?;
                }
                std::fs::copy(&absolute, &destination).map_err(|error| {
                    io::IOError::with_path(error, &absolute)
                })?;
            }
            Ok(())
        })
        .await
        .map_err(|_| {
            crate::Error::from(crate::ErrorKind::InputError(
                "Failed to copy resource pack".to_string(),
            ))
        })??;
    }

    let now = chrono::Utc::now();
    let mut copy = source;
    copy.id = new_id;
    copy.name = format!("{} (copy)", copy.name)
        .chars()
        .take(MAX_NAME_LENGTH)
        .collect();
    copy.created = now;
    copy.modified = now;
    copy.icon_path = copy
        .icon_path
        .as_ref()
        .map(|_| target_dir.join("pack.png"))
        .filter(|path| path.exists())
        .map(|path| path.to_string_lossy().to_string());

    super::writer::write_pack_mcmeta(&state, &copy).await?;
    index.packs.push(copy.clone());
    write_index(&state, &index).await?;

    Ok(copy)
}

/// Permanently delete a resource pack (its folder and its index entry).
pub async fn delete_resourcepack(id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let mut index = read_index(&state).await?;
    let position = index
        .packs
        .iter()
        .position(|pack| pack.id == id)
        .ok_or_else(unknown_pack_error)?;
    index.packs.remove(position);

    let dir = pack_dir(&state, id);
    if dir.exists() {
        io::remove_dir_all(&dir).await?;
    }
    write_index(&state, &index).await?;

    Ok(())
}

pub(super) fn unknown_pack_error() -> crate::Error {
    crate::ErrorKind::InputError("Unknown resource pack".to_string()).into()
}

pub(super) fn asset_path_within_pack(
    pack_dir: &Path,
    relative_asset_path: &str,
) -> crate::Result<PathBuf> {
    // Reject absolute paths, empty segments, and `..` traversal up front.
    let relative = Path::new(relative_asset_path);
    if relative.is_absolute()
        || relative_asset_path.is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid resource pack asset path".to_string(),
        )
        .into());
    }

    let joined = pack_dir.join(relative);
    // Belt-and-suspenders: ensure the resulting path is still inside
    // `pack_dir` after joining (guards against odd normal components on
    // some platforms).
    let normalized_pack_dir =
        io::canonicalize(pack_dir).unwrap_or_else(|_| pack_dir.to_path_buf());
    if let Some(parent) = joined.parent()
        && let Ok(canonical_parent) = io::canonicalize(parent)
        && !canonical_parent.starts_with(&normalized_pack_dir)
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid resource pack asset path".to_string(),
        )
        .into());
    }

    Ok(joined)
}
