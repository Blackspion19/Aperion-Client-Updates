use crate::api::Result;
use std::path::PathBuf;
use tauri::{Runtime, plugin::TauriPlugin};
use theseus::resourcepack::{self, ResourcePackMetadata, VanillaAsset};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("resourcepack")
        .invoke_handler(tauri::generate_handler![
            resourcepack_list,
            resourcepack_get,
            resourcepack_create,
            resourcepack_edit,
            resourcepack_delete,
            resourcepack_list_vanilla_assets,
            resourcepack_read_vanilla_asset,
            resourcepack_read_mod_assets,
            resourcepack_read_asset,
            resourcepack_set_texture,
            resourcepack_set_sound,
            resourcepack_set_icon,
            resourcepack_remove_asset,
            resourcepack_deploy_to_instance,
            resourcepack_remove_from_instance,
            resourcepack_is_deployed_to_instance,
            resourcepack_duplicate,
            resourcepack_retarget,
            resourcepack_export,
            resourcepack_import,
        ])
        .build()
}

#[tauri::command]
pub async fn resourcepack_list() -> Result<Vec<ResourcePackMetadata>> {
    Ok(resourcepack::list_resourcepacks().await?)
}

#[tauri::command]
pub async fn resourcepack_get(
    id: &str,
) -> Result<Option<ResourcePackMetadata>> {
    Ok(resourcepack::get_resourcepack(id).await?)
}

#[tauri::command]
pub async fn resourcepack_create(
    name: String,
    description: String,
    game_version: String,
    max_game_version: Option<String>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::create_resourcepack(
        name,
        description,
        game_version,
        max_game_version,
    )
    .await?)
}

#[tauri::command]
pub async fn resourcepack_edit(
    id: &str,
    name: Option<String>,
    description: Option<String>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::edit_resourcepack(id, name, description, None).await?)
}

#[tauri::command]
pub async fn resourcepack_delete(id: &str) -> Result<()> {
    Ok(resourcepack::delete_resourcepack(id).await?)
}

/// List every overridable vanilla texture/sound/font for the given game
/// version, so the editor can offer them for replacement. Requires that the
/// version has been launched/downloaded at least once already.
#[tauri::command]
pub async fn resourcepack_list_vanilla_assets(
    game_version: &str,
) -> Result<Vec<VanillaAsset>> {
    Ok(resourcepack::list_vanilla_assets(game_version).await?)
}

/// Read a vanilla asset's raw bytes (e.g. to show a before/after preview in
/// the editor).
#[tauri::command]
pub async fn resourcepack_read_vanilla_asset(
    game_version: &str,
    relative_asset_path: &str,
) -> Result<tauri::ipc::Response> {
    // `Response` sends raw bytes instead of a JSON number array, which is
    // much faster for the many thumbnails of the texture list.
    Ok(tauri::ipc::Response::new(
        resourcepack::read_vanilla_asset(game_version, relative_asset_path)
            .await?,
    ))
}

/// Read assets (blockstates, models, textures) out of the mod jars of an
/// instance. The answer is one length-prefixed block per requested path.
#[tauri::command]
pub async fn resourcepack_read_mod_assets(
    instance_id: &str,
    paths: Vec<String>,
) -> Result<tauri::ipc::Response> {
    Ok(tauri::ipc::Response::new(
        resourcepack::read_mod_assets(instance_id, paths).await?,
    ))
}

/// Read an asset's bytes back out of the pack itself, i.e. the replacement
/// the user already saved (not the vanilla original). Lets the editor show
/// the *current* override when the user opens an already-customized asset.
#[tauri::command]
pub async fn resourcepack_read_asset(
    pack_id: &str,
    relative_asset_path: &str,
) -> Result<tauri::ipc::Response> {
    Ok(tauri::ipc::Response::new(
        resourcepack::read_resourcepack_asset(pack_id, relative_asset_path)
            .await?,
    ))
}

#[tauri::command]
pub async fn resourcepack_set_texture(
    pack_id: &str,
    relative_asset_path: &str,
    png_bytes: Vec<u8>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::set_resourcepack_texture(
        pack_id,
        relative_asset_path,
        png_bytes,
    )
    .await?)
}

#[tauri::command]
pub async fn resourcepack_set_sound(
    pack_id: &str,
    relative_asset_path: &str,
    ogg_bytes: Vec<u8>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::set_resourcepack_sound(
        pack_id,
        relative_asset_path,
        ogg_bytes,
    )
    .await?)
}

#[tauri::command]
pub async fn resourcepack_set_icon(
    pack_id: &str,
    png_bytes: Vec<u8>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::set_resourcepack_icon(pack_id, png_bytes).await?)
}

#[tauri::command]
pub async fn resourcepack_remove_asset(
    pack_id: &str,
    relative_asset_path: &str,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::remove_resourcepack_asset(
        pack_id,
        relative_asset_path,
    )
    .await?)
}

#[tauri::command]
pub async fn resourcepack_deploy_to_instance(
    pack_id: &str,
    instance_id: &str,
) -> Result<()> {
    Ok(resourcepack::deploy_resourcepack_to_instance(pack_id, instance_id)
        .await?)
}

#[tauri::command]
pub async fn resourcepack_remove_from_instance(
    pack_id: &str,
    instance_id: &str,
) -> Result<()> {
    Ok(
        resourcepack::remove_resourcepack_from_instance(pack_id, instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn resourcepack_is_deployed_to_instance(
    pack_id: &str,
    instance_id: &str,
) -> Result<bool> {
    Ok(resourcepack::is_resourcepack_deployed_to_instance(
        pack_id,
        instance_id,
    )
    .await?)
}

#[tauri::command]
pub async fn resourcepack_duplicate(id: &str) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::duplicate_resourcepack(id).await?)
}

/// Change the Minecraft version a pack targets (updates `pack.mcmeta`).
#[tauri::command]
pub async fn resourcepack_retarget(
    id: &str,
    game_version: String,
    max_game_version: Option<String>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::retarget_resourcepack(id, game_version, max_game_version)
        .await?)
}

/// Export a pack as a `.zip` to the given path. Returns the written path.
#[tauri::command]
pub async fn resourcepack_export(
    pack_id: &str,
    destination: PathBuf,
) -> Result<PathBuf> {
    Ok(resourcepack::export_resourcepack(pack_id, destination).await?)
}

/// Import a resource pack `.zip` as a new pack.
#[tauri::command]
pub async fn resourcepack_import(
    zip_path: PathBuf,
    name: Option<String>,
    game_version: String,
    max_game_version: Option<String>,
) -> Result<ResourcePackMetadata> {
    Ok(resourcepack::import_resourcepack(
        zip_path,
        name,
        game_version,
        max_game_version,
    )
    .await?)
}
