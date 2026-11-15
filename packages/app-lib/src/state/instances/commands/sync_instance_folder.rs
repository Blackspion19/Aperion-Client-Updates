//! Keeps the folder of an instance in sync with its name and repairs the
//! absolute paths that Litematica / malilib store in their config files.

use super::create_instance::{resolve_instance_path, sanitize_instance_name};
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{InstanceInstallStage, State};
use crate::util::io;
use std::path::{Path, PathBuf};

/// True when `path` is the folder name for `desired`, either exactly or with
/// the numeric suffix that is added when the name is already taken ("Name (1)").
fn folder_name_matches(path: &str, desired: &str) -> bool {
    if path == desired {
        return true;
    }

    path.strip_prefix(desired)
        .and_then(|rest| rest.strip_prefix(" ("))
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|number| {
            !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
        })
}

/// Renames the folder of the instance to match its name. Returns true when the
/// folder was renamed.
pub(crate) async fn sync_instance_folder(
    instance_id: &str,
    state: &State,
) -> crate::Result<bool> {
    let Some(instance) =
        instance_rows::get_instance_by_id(instance_id, &state.pool).await?
    else {
        return Ok(false);
    };

    if !matches!(instance.install_stage, InstanceInstallStage::Installed) {
        return Ok(false);
    }

    let desired = sanitize_instance_name(&instance.name);
    if desired.is_empty() || folder_name_matches(&instance.path, &desired) {
        return Ok(false);
    }

    if crate::state::instance_has_running_process(instance_id, state).await? {
        return Ok(false);
    }

    // Never wait for this lock: games and installs hold it for a long time,
    // and waiting here would block everything behind it. Try again next start.
    let Ok(_lease) = state.content_store.runtime_cache_lock.try_write() else {
        return Ok(false);
    };

    let instances_dir = state.directories.instances_dir();
    let old_full = instances_dir.join(&instance.path);
    if !old_full.is_dir() {
        return Ok(false);
    }

    // Only the letter case changes: the name is "taken" by the folder itself.
    let case_only = instance.path.to_lowercase() == desired.to_lowercase();
    let new_path = if case_only {
        desired.clone()
    } else {
        resolve_instance_path(&instance.name, None, state).await?.0
    };
    let new_full = instances_dir.join(&new_path);

    io::rename_or_move(&old_full, &new_full).await?;

    if let Err(error) =
        sqlx::query("UPDATE instances SET path = ? WHERE id = ?")
            .bind(&new_path)
            .bind(instance_id)
            .execute(&state.pool)
            .await
    {
        // Put the folder back so database and disk stay consistent.
        let _ = io::rename_or_move(&new_full, &old_full).await;
        return Err(error.into());
    }

    crate::state::instances::watcher::rewatch_instance_folder(
        instance_id,
        &instance.path,
        &new_path,
        &state.file_watcher,
        &state.directories,
    )
    .await;

    tracing::info!(
        "Renamed instance folder '{}' to '{}'",
        instance.path,
        new_path
    );

    let old_dir = old_full.clone();
    let new_dir = new_full.clone();
    let instances_dir_owned = instances_dir.clone();
    let _ = tokio::task::spawn_blocking(move || {
        fix_litematica_config(
            &new_dir,
            &instances_dir_owned,
            Some(&old_dir),
            &new_path,
        );
    })
    .await;

    Ok(true)
}

/// Startup check: every instance folder should match the instance name, and
/// Litematica configs must point into the folder they live in.
pub(crate) async fn sync_all_instance_folders(state: &State) {
    let Ok(mut instances) = instance_rows::list_instances(&state.pool).await
    else {
        return;
    };
    instances.sort_by_key(|instance| instance.created);

    for instance in instances {
        if let Err(error) = sync_instance_folder(&instance.id, state).await {
            tracing::warn!(
                "Failed to sync folder name of instance {}: {error}",
                instance.id
            );
        }

        let Ok(Some(current)) =
            instance_rows::get_instance_by_id(&instance.id, &state.pool).await
        else {
            continue;
        };
        let instances_dir = state.directories.instances_dir();
        let instance_dir = instances_dir.join(&current.path);
        let folder = current.path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            fix_litematica_config(&instance_dir, &instances_dir, None, &folder);
        })
        .await;
    }
}

fn has_litematica(instance_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(instance_dir.join("mods")) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        name.contains("litematica")
            && (name.ends_with(".jar") || name.ends_with(".jar.disabled"))
    })
}

fn collect_json_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_json_files(&path, depth + 1, out);
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            out.push(path);
        }
    }
}

/// The spellings of a path as it can appear inside a JSON file: with escaped
/// backslashes (Windows) and with forward slashes.
fn json_path_variants(path: &str) -> [String; 2] {
    [path.replace('\\', "\\\\"), path.replace('\\', "/")]
}

const PATH_END_MARKERS: [&str; 3] = ["\\\\", "/", "\""];

/// Fixes the absolute paths in `config/litematica` and `config/malilib`.
///
/// With `old_dir` set, paths into that folder are moved to `instance_dir`.
/// Without it, paths into folders of the instances directory that no longer
/// exist are pointed to `instance_dir`.
fn fix_litematica_config(
    instance_dir: &Path,
    instances_dir: &Path,
    old_dir: Option<&Path>,
    current_folder: &str,
) {
    if !has_litematica(instance_dir) {
        return;
    }

    let mut files = Vec::new();
    for sub_dir in ["litematica", "malilib"] {
        collect_json_files(&instance_dir.join("config").join(sub_dir), 0, &mut files);
    }

    let new_variants = json_path_variants(&instance_dir.to_string_lossy());
    let old_variants =
        old_dir.map(|dir| json_path_variants(&dir.to_string_lossy()));
    let dir_variants = json_path_variants(&instances_dir.to_string_lossy());

    for file in files {
        let Ok(metadata) = std::fs::metadata(&file) else {
            continue;
        };
        if metadata.len() > 20 * 1024 * 1024 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };

        let mut result = text.clone();

        if let Some(old_variants) = &old_variants {
            for (old, new) in old_variants.iter().zip(new_variants.iter()) {
                for marker in PATH_END_MARKERS {
                    let from = format!("{old}{marker}");
                    if result.contains(&from) {
                        result = result.replace(&from, &format!("{new}{marker}"));
                    }
                }
            }
        } else {
            for (index, dir_variant) in dir_variants.iter().enumerate() {
                let separator = if index == 0 && dir_variant.contains("\\\\") {
                    "\\\\"
                } else {
                    "/"
                };
                let prefix = format!("{dir_variant}{separator}");
                for stale in stale_folders(
                    &result,
                    &prefix,
                    instances_dir,
                    current_folder,
                ) {
                    for marker in PATH_END_MARKERS {
                        let from = format!("{prefix}{stale}{marker}");
                        let to = format!("{prefix}{current_folder}{marker}");
                        result = result.replace(&from, &to);
                    }
                }
            }
        }

        if result != text {
            match std::fs::write(&file, result) {
                Ok(()) => tracing::info!(
                    "Updated instance paths in {}",
                    file.display()
                ),
                Err(error) => tracing::warn!(
                    "Failed to update instance paths in {}: {error}",
                    file.display()
                ),
            }
        }
    }
}

/// Folder names that appear behind `prefix` in `text` but do not exist (any
/// more) in the instances directory.
fn stale_folders(
    text: &str,
    prefix: &str,
    instances_dir: &Path,
    current_folder: &str,
) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;

    while let Some(position) = rest.find(prefix) {
        let after = &rest[position + prefix.len()..];
        rest = after;

        let end = after.find(['"', '/', '\\']).unwrap_or(after.len());
        let candidate = &after[..end];
        let tail = &after[end..];

        // A lone backslash is a JSON escape sequence (like \u0027), not the
        // end of the folder name.
        if tail.starts_with('\\') && !tail.starts_with("\\\\") {
            continue;
        }

        if candidate.is_empty()
            || candidate == current_folder
            || found.iter().any(|existing| existing == candidate)
            || instances_dir.join(candidate).exists()
        {
            continue;
        }

        found.push(candidate.to_string());
    }

    found
}
