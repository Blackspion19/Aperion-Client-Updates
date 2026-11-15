use crate::api::Result;
use theseus::aperion_ingame::{self, IngameInfo};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("aperion-ingame")
        .invoke_handler(tauri::generate_handler![
            aperion_ingame_info,
            aperion_ingame_prefetch,
        ])
        .build()
}

/// Which ingame build exists for a Minecraft version (local and on GitHub).
#[tauri::command]
pub async fn aperion_ingame_info(game_version: String) -> Result<IngameInfo> {
    Ok(aperion_ingame::info(&game_version).await?)
}

/// Starts fetching the jar right after the user turned the client on.
#[tauri::command]
pub async fn aperion_ingame_prefetch(
    game_version: String,
) -> Result<IngameInfo> {
    Ok(aperion_ingame::prefetch(&game_version).await?)
}
