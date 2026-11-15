use super::repository::{
    asset_path_within_pack, pack_dir, touch_pack_assets, unknown_pack_error,
};
use super::{ResourcePackMetadata, get_resourcepack};
use crate::state::State;
use crate::util::io;
use image::ImageFormat;
use std::io::Cursor;

const MAX_TEXTURE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEXTURE_DIMENSION: u32 = 4096;
const MAX_SOUND_BYTES: usize = 32 * 1024 * 1024;
const MAX_ICON_BYTES: usize = 4 * 1024 * 1024;

/// Read a single overridden asset's raw bytes back out of the pack (i.e.
/// the replacement the user saved, not the vanilla original). Used by the
/// editor to show what the override currently looks like, so clicking an
/// already-customized item/block jumps straight to *its* editor showing
/// *its* current texture — not a generic, unrelated picker.
pub async fn read_resourcepack_asset(
    pack_id: &str,
    relative_asset_path: &str,
) -> crate::Result<Vec<u8>> {
    let state = State::get().await?;
    let dir = pack_dir(&state, pack_id);
    let asset_path = asset_path_within_pack(&dir, relative_asset_path)?;
    Ok(io::read(&asset_path).await?)
}

pub(super) async fn init_pack_folder(
    state: &State,
    metadata: &ResourcePackMetadata,
) -> crate::Result<()> {
    let dir = pack_dir(state, &metadata.id);
    io::create_dir_all(&dir).await?;
    io::create_dir_all(dir.join("assets").join("minecraft")).await?;
    write_pack_mcmeta(state, metadata).await
}

pub(super) async fn write_pack_mcmeta(
    state: &State,
    metadata: &ResourcePackMetadata,
) -> crate::Result<()> {
    let dir = pack_dir(state, &metadata.id);
    io::create_dir_all(&dir).await?;

    let min = super::PackFormat(metadata.pack_format);
    let max = match metadata.max_game_version.as_deref() {
        // No upper limit: this version and newer
        Some("*") => None,
        Some(version) => Some(super::PackFormat::for_game_version(version)),
        // Only the one version
        None => Some(min),
    };
    let mcmeta = super::PackFormat::mcmeta_range(min, max, &metadata.description);
    let bytes = serde_json::to_vec_pretty(&mcmeta)?;
    io::write(dir.join("pack.mcmeta"), &bytes).await?;
    Ok(())
}

/// Replace (override) a single texture asset in the pack, e.g.
/// `assets/minecraft/textures/block/stone.png`. The provided bytes must be a
/// valid PNG; they're re-encoded to normalize the format and enforce size
/// limits, mirroring how instance icons are handled.
pub async fn set_resourcepack_texture(
    pack_id: &str,
    relative_asset_path: &str,
    png_bytes: Vec<u8>,
) -> crate::Result<ResourcePackMetadata> {
    if !relative_asset_path.ends_with(".png") {
        return Err(crate::ErrorKind::InputError(
            "Resource pack textures must be PNG files".to_string(),
        )
        .into());
    }
    if png_bytes.len() > MAX_TEXTURE_BYTES {
        return Err(crate::ErrorKind::InputError(
            "Resource pack texture is too large".to_string(),
        )
        .into());
    }

    let normalized = tokio::task::spawn_blocking(move || {
        normalize_texture(&png_bytes)
    })
    .await
    .map_err(|_| {
        crate::Error::from(crate::ErrorKind::InputError(
            "Failed to process resource pack texture".to_string(),
        ))
    })??;

    set_resourcepack_asset(pack_id, relative_asset_path, normalized).await
}

/// Replace (override) a single sound asset in the pack, e.g.
/// `assets/minecraft/sounds/block/stone/break1.ogg`. Bytes are written
/// as-is (Minecraft expects Ogg Vorbis; validating the codec is left to the
/// game itself, same as the vanilla launcher does for resource packs).
pub async fn set_resourcepack_sound(
    pack_id: &str,
    relative_asset_path: &str,
    ogg_bytes: Vec<u8>,
) -> crate::Result<ResourcePackMetadata> {
    if !relative_asset_path.ends_with(".ogg") {
        return Err(crate::ErrorKind::InputError(
            "Resource pack sounds must be Ogg Vorbis (.ogg) files".to_string(),
        )
        .into());
    }
    if !ogg_bytes.starts_with(b"OggS") {
        return Err(crate::ErrorKind::InputError(
            "This file is not a valid Ogg Vorbis (.ogg) sound".to_string(),
        )
        .into());
    }
    if ogg_bytes.len() > MAX_SOUND_BYTES {
        return Err(crate::ErrorKind::InputError(
            "Resource pack sound file is too large".to_string(),
        )
        .into());
    }

    set_resourcepack_asset(pack_id, relative_asset_path, ogg_bytes).await
}

/// Remove an override, reverting that asset back to vanilla.
pub async fn remove_resourcepack_asset(
    pack_id: &str,
    relative_asset_path: &str,
) -> crate::Result<ResourcePackMetadata> {
    let state = State::get().await?;
    let dir = pack_dir(&state, pack_id);
    let asset_path = asset_path_within_pack(&dir, relative_asset_path)?;

    if asset_path.exists() {
        io::remove_file(&asset_path).await?;
    }

    let mut metadata =
        get_resourcepack(pack_id).await?.ok_or_else(unknown_pack_error)?;
    metadata
        .overridden_assets
        .retain(|path| path != relative_asset_path);

    touch_pack_assets(&state, pack_id, metadata.overridden_assets).await
}

/// Set the pack's icon (`pack.png`), a 128x128-or-smaller PNG shown in the
/// in-game resource pack list.
pub async fn set_resourcepack_icon(
    pack_id: &str,
    png_bytes: Vec<u8>,
) -> crate::Result<ResourcePackMetadata> {
    if png_bytes.len() > MAX_ICON_BYTES {
        return Err(crate::ErrorKind::InputError(
            "Resource pack icon is too large".to_string(),
        )
        .into());
    }

    let normalized =
        tokio::task::spawn_blocking(move || normalize_texture(&png_bytes))
            .await
            .map_err(|_| {
                crate::Error::from(crate::ErrorKind::InputError(
                    "Failed to process resource pack icon".to_string(),
                ))
            })??;

    let state = State::get().await?;
    let dir = pack_dir(&state, pack_id);
    if !dir.exists() {
        return Err(unknown_pack_error());
    }
    io::write(dir.join("pack.png"), &normalized).await?;

    let icon_path = dir.join("pack.png").to_string_lossy().to_string();
    super::repository::edit_resourcepack(
        pack_id,
        None,
        None,
        Some(Some(icon_path)),
    )
    .await
}

async fn set_resourcepack_asset(
    pack_id: &str,
    relative_asset_path: &str,
    bytes: Vec<u8>,
) -> crate::Result<ResourcePackMetadata> {
    let state = State::get().await?;
    let dir = pack_dir(&state, pack_id);
    if !dir.exists() {
        return Err(unknown_pack_error());
    }
    let asset_path = asset_path_within_pack(&dir, relative_asset_path)?;
    if let Some(parent) = asset_path.parent() {
        io::create_dir_all(parent).await?;
    }
    io::write(&asset_path, &bytes).await?;

    let mut metadata =
        get_resourcepack(pack_id).await?.ok_or_else(unknown_pack_error)?;
    if !metadata
        .overridden_assets
        .iter()
        .any(|path| path == relative_asset_path)
    {
        metadata.overridden_assets.push(relative_asset_path.to_string());
    }

    touch_pack_assets(&state, pack_id, metadata.overridden_assets).await
}

fn normalize_texture(bytes: &[u8]) -> crate::Result<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| input_error(error.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_TEXTURE_DIMENSION);
    limits.max_image_height = Some(MAX_TEXTURE_DIMENSION);
    reader.limits(limits);

    let image = reader.decode().map_err(|error| input_error(error.to_string()))?;
    let mut encoded = Cursor::new(Vec::new());
    image
        .to_rgba8()
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|error| input_error(error.to_string()))?;
    Ok(encoded.into_inner())
}

fn input_error(message: String) -> crate::Error {
    crate::ErrorKind::InputError(message).into()
}
