//! Integration of the Aperion ingame client (a Fabric mod) into the launcher.
//!
//! * The mod jars are published by hand as GitHub release assets, one jar per
//!   Minecraft version, named `aperion-ingame-<modversion>-mc<mcversion>.jar`
//!   (e.g. `aperion-ingame-0.2.0-mc26.2.jar`), on releases whose tag starts
//!   with `ingame-`.
//! * The launcher keeps the jars in its own data folder and never touches the
//!   instance's `mods` folder. At launch the jar is handed to Fabric through
//!   `-Dfabric.addMods`.
//! * Updates are downloaded silently in the background. A jar that is in use
//!   is never overwritten: new versions get a new file name and are picked up
//!   on the next game start.
//! * The Fabric API (required by the mod) is added the same way, but only if
//!   the instance does not already ship it.

use crate::State;
use crate::util::fetch::REQWEST_CLIENT;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// GitHub repository the ingame jars are published in (`owner/name`).
/// Can be overridden with the `APERION_INGAME_REPO` environment variable.
pub const DEFAULT_GITHUB_REPO: &str = "Blackspion19/Aperion-Client-Updates";
/// Only releases with a tag starting with this prefix are considered.
pub const RELEASE_TAG_PREFIX: &str = "ingame-";

const ASSET_PREFIX: &str = "aperion-ingame-";
const MOD_ID: &str = "aperion";
const FABRIC_API_ID: &str = "fabric-api";
const UPDATE_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);
const INDEX_MAX_AGE: Duration = Duration::from_secs(15 * 60);
const KEEP_VERSIONS: usize = 2;
const MAX_JAR_BYTES: usize = 64 * 1024 * 1024;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn err(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::LauncherError(message.into()).as_error()
}

fn repo() -> String {
    std::env::var("APERION_INGAME_REPO")
        .ok()
        .filter(|r| r.contains('/'))
        .unwrap_or_else(|| DEFAULT_GITHUB_REPO.to_owned())
}

/// A build published on GitHub.
#[derive(Clone, Debug)]
struct RemoteBuild {
    version: String,
    url: String,
    sha256: Option<String>,
}

#[derive(Default)]
struct RemoteIndex {
    builds: BTreeMap<String, RemoteBuild>,
    etag: Option<String>,
    last_check: Option<Instant>,
}

static REMOTE: LazyLock<Mutex<RemoteIndex>> =
    LazyLock::new(|| Mutex::new(RemoteIndex::default()));

/// Serializes downloads so the background updater and a launch never race.
static DOWNLOAD_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Clone, Debug, Serialize)]
pub struct IngameInfo {
    pub game_version: String,
    /// Newest version available on this computer.
    pub installed_version: Option<String>,
    /// Newest version published on GitHub (if the index could be fetched).
    pub latest_version: Option<String>,
    /// A jar exists locally or on GitHub for this Minecraft version.
    pub available: bool,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

/// `aperion-ingame-0.2.0-mc26.2.jar` -> `("0.2.0", "26.2")`
fn parse_asset_name(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix(ASSET_PREFIX)?.strip_suffix(".jar")?;
    let (version, mc) = rest.split_once("-mc")?;
    // rejects e.g. "...-mc26.2-sources.jar"
    let valid = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    };
    (valid(version) && valid(mc)).then(|| (version.to_owned(), mc.to_owned()))
}

fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

fn is_newer(candidate: &str, than: &str) -> bool {
    version_key(candidate) > version_key(than)
}

/// Only plain folder-name characters may be used as path components.
fn safe_component(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        && !s.starts_with('.')
}

/// Like [`safe_component`], but also allows `+` (Fabric API file names look
/// like `fabric-api-0.154.0+26.2.jar`).
fn safe_file_name(s: &str) -> bool {
    !s.is_empty()
        && !s.contains("..")
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+')
        })
        && !s.starts_with('.')
}

async fn base_dir() -> crate::Result<PathBuf> {
    Ok(State::get().await?.directories.aperion_ingame_dir())
}

fn mod_jar_name(version: &str, mc: &str) -> String {
    format!("{ASSET_PREFIX}{version}-mc{mc}.jar")
}

/// All jars cached locally for a Minecraft version, oldest first.
async fn local_builds(mc: &str) -> Vec<(String, PathBuf)> {
    if !safe_component(mc) {
        return Vec::new();
    }
    let Ok(base) = base_dir().await else {
        return Vec::new();
    };
    let dir = base.join("mods").join(mc);
    let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
        return Vec::new();
    };
    let mut found = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if let Some((version, jar_mc)) = parse_asset_name(&file_name)
            && jar_mc == mc
        {
            found.push((version, entry.path()));
        }
    }
    found.sort_by(|a, b| version_key(&a.0).cmp(&version_key(&b.0)));
    found
}

async fn latest_local(mc: &str) -> Option<(String, PathBuf)> {
    local_builds(mc).await.pop()
}

fn remote_build(mc: &str) -> Option<RemoteBuild> {
    REMOTE.lock().ok()?.builds.get(mc).cloned()
}

// ---------------------------------------------------------------------------
// GitHub index
// ---------------------------------------------------------------------------

async fn refresh_remote_index(force: bool) -> crate::Result<()> {
    let (etag, has_builds, fresh) = {
        let remote = REMOTE.lock().map_err(|_| err("index lock poisoned"))?;
        (
            remote.etag.clone(),
            !remote.builds.is_empty(),
            remote
                .last_check
                .is_some_and(|checked| checked.elapsed() < INDEX_MAX_AGE),
        )
    };
    if fresh && !force {
        return Ok(());
    }

    let mut request = REQWEST_CLIENT
        .get(format!(
            "https://api.github.com/repos/{}/releases?per_page=30",
            repo()
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(Duration::from_secs(15));
    if let Some(etag) = etag
        && has_builds
    {
        request = request.header("If-None-Match", etag);
    }

    let response = request.send().await?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        if let Ok(mut remote) = REMOTE.lock() {
            remote.last_check = Some(Instant::now());
        }
        return Ok(());
    }
    if !status.is_success() {
        return Err(err(format!("GitHub releases request failed: {status}")));
    }
    let new_etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let releases: Vec<GhRelease> = response.json().await?;

    let mut builds: BTreeMap<String, RemoteBuild> = BTreeMap::new();
    for release in releases {
        if release.draft
            || release.prerelease
            || !release.tag_name.starts_with(RELEASE_TAG_PREFIX)
        {
            continue;
        }
        for asset in release.assets {
            let Some((version, mc)) = parse_asset_name(&asset.name) else {
                continue;
            };
            let better = builds
                .get(&mc)
                .is_none_or(|existing| is_newer(&version, &existing.version));
            if better {
                let sha256 = asset
                    .digest
                    .as_deref()
                    .and_then(|d| d.strip_prefix("sha256:"))
                    .map(|d| d.to_ascii_lowercase());
                builds.insert(
                    mc,
                    RemoteBuild {
                        version,
                        url: asset.browser_download_url,
                        sha256,
                    },
                );
            }
        }
    }

    let mut remote = REMOTE.lock().map_err(|_| err("index lock poisoned"))?;
    remote.builds = builds;
    remote.etag = new_etag;
    remote.last_check = Some(Instant::now());
    Ok(())
}

// ---------------------------------------------------------------------------
// Download + verification
// ---------------------------------------------------------------------------

/// The jar must be a real mod with the expected id (guards against a wrong or
/// broken release asset).
fn jar_has_mod_id(bytes: &[u8], expected_id: &str) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes))
    else {
        return false;
    };
    let Ok(mut file) = archive.by_name("fabric.mod.json") else {
        return false;
    };
    let mut text = String::new();
    if std::io::Read::read_to_string(&mut file, &mut text).is_err() {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str().map(str::to_owned)))
        .is_some_and(|id| id == expected_id)
}

async fn write_atomic(target: &Path, bytes: &[u8]) -> crate::Result<()> {
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = target.with_extension(format!("part-{}", Uuid::new_v4().simple()));
    tokio::fs::write(&tmp, bytes).await?;
    if let Err(e) = tokio::fs::rename(&tmp, target).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    Ok(())
}

async fn download_bytes(url: &str, timeout: Duration) -> crate::Result<Vec<u8>> {
    let response = REQWEST_CLIENT
        .get(url)
        .timeout(timeout)
        .send()
        .await?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|len| len as usize > MAX_JAR_BYTES)
    {
        return Err(err("Download is unexpectedly large"));
    }
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_JAR_BYTES {
        return Err(err("Download is unexpectedly large"));
    }
    Ok(bytes.to_vec())
}

async fn prune_old_builds(mc: &str) {
    let builds = local_builds(mc).await;
    if builds.len() <= KEEP_VERSIONS {
        return;
    }
    for (_, path) in &builds[..builds.len() - KEEP_VERSIONS] {
        // May fail on Windows while a running game still holds the file.
        let _ = tokio::fs::remove_file(path).await;
    }
}

async fn download_remote_build(
    mc: &str,
    build: &RemoteBuild,
) -> crate::Result<PathBuf> {
    if !safe_component(mc) || !safe_component(&build.version) {
        return Err(err("Invalid version in release asset"));
    }
    let _guard = DOWNLOAD_LOCK.lock().await;
    let target = base_dir()
        .await?
        .join("mods")
        .join(mc)
        .join(mod_jar_name(&build.version, mc));
    if target.exists() {
        return Ok(target);
    }

    let bytes = download_bytes(&build.url, Duration::from_secs(120)).await?;

    if let Some(expected) = &build.sha256 {
        let actual = to_hex(&Sha256::digest(&bytes));
        if &actual != expected {
            return Err(err("Aperion ingame download failed its checksum"));
        }
    }
    let for_check = bytes.clone();
    let valid = tokio::task::spawn_blocking(move || {
        jar_has_mod_id(&for_check, MOD_ID)
    })
    .await
    .unwrap_or(false);
    if !valid {
        return Err(err("Downloaded file is not the Aperion ingame mod"));
    }

    write_atomic(&target, &bytes).await?;
    tracing::info!(
        "[APERION] Downloaded Aperion ingame {} for Minecraft {mc}",
        build.version
    );
    prune_old_builds(mc).await;
    Ok(target)
}

/// Returns the newest jar for a Minecraft version, downloading it first if
/// GitHub has a newer one (or none is cached yet). Never fails: if anything
/// goes wrong the newest cached jar (if any) is returned.
async fn ensure_latest(mc: &str) -> Option<(String, PathBuf)> {
    let local = latest_local(mc).await;
    if let Err(e) = refresh_remote_index(false).await {
        tracing::debug!("[APERION] Could not check for ingame updates: {e}");
    }
    if let Some(remote) = remote_build(mc) {
        let needs_download = local
            .as_ref()
            .is_none_or(|(version, _)| is_newer(&remote.version, version));
        if needs_download {
            match download_remote_build(mc, &remote).await {
                Ok(path) => return Some((remote.version, path)),
                Err(e) => tracing::warn!(
                    "[APERION] Could not download Aperion ingame {}: {e}",
                    remote.version
                ),
            }
        }
    }
    local
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Status for the instance header button. Refreshes the GitHub index when it
/// is stale; does not download anything.
pub async fn info(game_version: &str) -> crate::Result<IngameInfo> {
    if let Err(e) = refresh_remote_index(false).await {
        tracing::debug!("[APERION] Could not check for ingame builds: {e}");
    }
    let installed = latest_local(game_version).await.map(|(v, _)| v);
    let latest = remote_build(game_version).map(|b| b.version);
    Ok(IngameInfo {
        game_version: game_version.to_owned(),
        available: installed.is_some() || latest.is_some(),
        installed_version: installed,
        latest_version: latest,
    })
}

/// Called when the user turns the ingame client on: fetches the jar (and the
/// Fabric API) right away so the next start does not have to wait.
pub async fn prefetch(game_version: &str) -> crate::Result<IngameInfo> {
    ensure_latest(game_version).await;
    ensure_fabric_api(game_version).await;
    info(game_version).await
}

/// One round of silent updates for every Minecraft version that is cached.
pub async fn sync_updates() {
    let Ok(base) = base_dir().await else {
        return;
    };
    let Ok(mut entries) = tokio::fs::read_dir(base.join("mods")).await else {
        return;
    };
    if let Err(e) = refresh_remote_index(true).await {
        tracing::debug!("[APERION] Update check failed: {e}");
        return;
    }
    while let Ok(Some(entry)) = entries.next_entry().await {
        let mc = entry.file_name().to_string_lossy().into_owned();
        if !safe_component(&mc) {
            continue;
        }
        ensure_latest(&mc).await;
    }
}

/// Endless background loop; spawn once at app start.
pub async fn run_background_updater() {
    static STARTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    // Let the app finish starting first.
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        sync_updates().await;
        tokio::time::sleep(UPDATE_INTERVAL).await;
    }
}

/// What the launcher needs to add to the game start.
pub struct LaunchPlan {
    pub jvm_args: Vec<String>,
    /// Folder shared with the mod for session renewal (`request.json` /
    /// `session.json`).
    pub session_dir: PathBuf,
}

/// Looks at the instance's `mods` folder: (has our mod, has the Fabric API).
async fn scan_mods(instance_path: &Path) -> (bool, bool) {
    let mods_dir = instance_path.join("mods");
    tokio::task::spawn_blocking(move || {
        let (mut has_mod, mut has_api) = (false, false);
        let Ok(entries) = std::fs::read_dir(&mods_dir) else {
            return (false, false);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_jar = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jar"));
            if !is_jar {
                continue;
            }
            let Ok(file) = std::fs::File::open(&path) else {
                continue;
            };
            let Ok(mut archive) = zip::ZipArchive::new(file) else {
                continue;
            };
            let Ok(mut meta) = archive.by_name("fabric.mod.json") else {
                continue;
            };
            let mut text = String::new();
            if std::io::Read::read_to_string(&mut meta, &mut text).is_err() {
                continue;
            }
            let id = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| {
                    v.get("id").and_then(|id| id.as_str().map(str::to_owned))
                });
            match id.as_deref() {
                Some(MOD_ID) => has_mod = true,
                Some(FABRIC_API_ID) => has_api = true,
                _ => {}
            }
        }
        (has_mod, has_api)
    })
    .await
    .unwrap_or((false, false))
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: BTreeMap<String, String>,
}

/// Newest cached Fabric API jar for a Minecraft version, downloading a newer
/// one from Modrinth when possible.
async fn ensure_fabric_api(mc: &str) -> Option<PathBuf> {
    if !safe_component(mc) {
        return None;
    }
    let dir = base_dir().await.ok()?.join("fabric-api").join(mc);

    match fetch_fabric_api(mc, &dir).await {
        Ok(path) => return Some(path),
        Err(e) => {
            tracing::warn!("[APERION] Could not fetch Fabric API: {e}");
        }
    }

    // Offline / failed: use the newest cached jar.
    let mut entries = tokio::fs::read_dir(&dir).await.ok()?;
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jar") {
            continue;
        }
        let modified = entry
            .metadata()
            .await
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

async fn fetch_fabric_api(mc: &str, dir: &Path) -> crate::Result<PathBuf> {
    let versions: Vec<ModrinthVersion> = REQWEST_CLIENT
        .get("https://api.modrinth.com/v2/project/fabric-api/version")
        .query(&[
            ("game_versions", format!("[\"{mc}\"]")),
            ("loaders", "[\"fabric\"]".to_owned()),
        ])
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Modrinth returns newest first. Prefer a release.
    let version = versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first())
        .ok_or_else(|| err(format!("No Fabric API build for {mc}")))?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| err("Fabric API version has no files"))?;

    if !safe_file_name(&file.filename) || !file.filename.ends_with(".jar") {
        return Err(err("Unexpected Fabric API file name"));
    }
    let target = dir.join(&file.filename);
    if target.exists() {
        return Ok(target);
    }

    let _guard = DOWNLOAD_LOCK.lock().await;
    let bytes = download_bytes(&file.url, Duration::from_secs(120)).await?;
    if let Some(expected) = file.hashes.get("sha512") {
        let actual = to_hex(&Sha512::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(err("Fabric API download failed its checksum"));
        }
    }
    let for_check = bytes.clone();
    let valid = tokio::task::spawn_blocking(move || {
        jar_has_mod_id(&for_check, FABRIC_API_ID)
    })
    .await
    .unwrap_or(false);
    if !valid {
        return Err(err("Downloaded file is not the Fabric API"));
    }
    write_atomic(&target, &bytes).await?;

    // Keep only the newest Fabric API jar around.
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path != target
                && path.extension().is_some_and(|e| e == "jar")
            {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
    }
    tracing::info!("[APERION] Downloaded Fabric API {}", file.filename);
    Ok(target)
}

fn path_list_separator() -> &'static str {
    if cfg!(windows) { ";" } else { ":" }
}

/// Prepares everything needed to start `game_version` with the ingame client.
/// Returns `None` when it cannot or should not be injected; the game then
/// simply starts without it.
pub async fn prepare_launch(
    game_version: &str,
    instance_path: &Path,
) -> Option<LaunchPlan> {
    let (has_mod, has_api) = scan_mods(instance_path).await;
    if has_mod {
        tracing::info!(
            "[APERION] An Aperion jar is already in the mods folder, not injecting another one"
        );
        return None;
    }

    // Use what we have right away; look for updates in the background so a
    // slow network never delays the game start.
    let jar = match latest_local(game_version).await {
        Some((_, path)) => {
            let mc = game_version.to_owned();
            tokio::spawn(async move {
                ensure_latest(&mc).await;
            });
            path
        }
        None => {
            // First launch: try to fetch it, but don't wait forever.
            match tokio::time::timeout(
                Duration::from_secs(45),
                ensure_latest(game_version),
            )
            .await
            {
                Ok(Some((_, path))) => path,
                _ => {
                    tracing::warn!(
                        "[APERION] No ingame build available for Minecraft {game_version}; starting without it"
                    );
                    return None;
                }
            }
        }
    };

    let mut mods = vec![jar];
    if !has_api {
        match ensure_fabric_api(game_version).await {
            Some(api) => mods.push(api),
            None => {
                tracing::warn!(
                    "[APERION] Fabric API is missing and could not be downloaded; starting without the ingame client"
                );
                return None;
            }
        }
    }

    let session_dir = base_dir()
        .await
        .ok()?
        .join("session")
        .join(Uuid::new_v4().simple().to_string());
    if tokio::fs::create_dir_all(&session_dir).await.is_err() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(
            &session_dir,
            std::fs::Permissions::from_mode(0o700),
        )
        .await;
    }

    let add_mods = mods
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(path_list_separator());

    Some(LaunchPlan {
        jvm_args: vec![
            format!("-Dfabric.addMods={add_mods}"),
            format!("-Daperion.session.dir={}", session_dir.to_string_lossy()),
        ],
        session_dir,
    })
}

/// Serves the mod's session-renewal requests while the game process runs.
///
/// The mod writes `request.json` (no token inside); we answer with a freshly
/// refreshed `session.json`. Tokens are never logged. The folder is removed
/// when the game exits.
pub fn spawn_session_bridge(
    session_dir: PathBuf,
    process_id: Uuid,
    account_id: Uuid,
) {
    tokio::spawn(async move {
        let request = session_dir.join("request.json");
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let Ok(state) = State::get().await else {
                break;
            };
            if state.process_manager.get(process_id).is_none() {
                break;
            }
            if !request.exists() {
                continue;
            }
            let _ = tokio::fs::remove_file(&request).await;

            let credentials = match crate::state::Credentials::get_default_credential(
                &state.pool,
            )
            .await
            {
                Ok(Some(credentials)) => credentials,
                Ok(None) => continue,
                Err(_) => {
                    tracing::warn!("[APERION] Session renewal failed");
                    continue;
                }
            };
            if credentials.offline_profile.id != account_id {
                tracing::warn!(
                    "[APERION] Session renewal skipped: active account changed"
                );
                continue;
            }

            let payload = serde_json::json!({
                "name": credentials.offline_profile.name,
                "uuid": credentials.offline_profile.id.hyphenated().to_string(),
                "accessToken": credentials.access_token,
            });
            let tmp = session_dir.join("session.json.tmp");
            let target = session_dir.join("session.json");
            let written = async {
                tokio::fs::write(&tmp, payload.to_string()).await?;
                tokio::fs::rename(&tmp, &target).await
            }
            .await;
            if written.is_err() {
                tracing::warn!("[APERION] Could not write session file");
            }
        }
        let _ = tokio::fs::remove_dir_all(&session_dir).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_asset_names() {
        assert_eq!(
            parse_asset_name("aperion-ingame-0.2.0-mc26.2.jar"),
            Some(("0.2.0".into(), "26.2".into()))
        );
        assert_eq!(
            parse_asset_name("aperion-ingame-1.0.0-mc1.21.4.jar"),
            Some(("1.0.0".into(), "1.21.4".into()))
        );
        assert_eq!(
            parse_asset_name("aperion-ingame-0.2.0-mc26.2-sources.jar"),
            None
        );
        assert_eq!(parse_asset_name("aperion-ingame-0.2.0.jar"), None);
        assert_eq!(parse_asset_name("other-0.2.0-mc26.2.jar"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
    }

    #[test]
    fn accepts_fabric_api_file_names() {
        assert!(safe_file_name("fabric-api-0.154.0+26.2.jar"));
        assert!(!safe_file_name("../evil.jar"));
        assert!(!safe_file_name("a/b.jar"));
    }

    #[test]
    fn rejects_unsafe_path_components() {
        assert!(safe_component("26.2"));
        assert!(!safe_component("../x"));
        assert!(!safe_component(""));
        assert!(!safe_component(".hidden"));
    }
}
