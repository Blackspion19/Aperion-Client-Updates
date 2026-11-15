//! Public API for creating and managing user-made resource packs, and for
//! deploying them into instances.
//!
//! Unlike mods and modpacks (which are sourced from Modrinth), resource
//! packs here are authored locally by the user: they pick a vanilla texture
//! or sound from an installed Minecraft version and replace it, and the
//! result is written out as a resource pack that can be deployed into any
//! instance running a compatible game version.

pub use crate::state::resourcepacks::{
    ResourcePackMetadata, VanillaAsset, VanillaAssetKind, create_resourcepack,
    delete_resourcepack, deploy_resourcepack_to_instance,
    duplicate_resourcepack, edit_resourcepack, export_resourcepack,
    get_resourcepack, import_resourcepack,
    is_resourcepack_deployed_to_instance,
    list_resourcepacks, list_vanilla_assets, read_resourcepack_asset,
    read_mod_assets, read_vanilla_asset, remove_resourcepack_asset,
    remove_resourcepack_from_instance, retarget_resourcepack,
    set_resourcepack_icon,
    set_resourcepack_sound, set_resourcepack_texture,
};
