mod archive_import;
mod download_manager;
mod game_sources;
mod windows_vpn;

use chrono::{DateTime, Utc};
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    env,
    fs,
    path::{Path, PathBuf},
    hash::{Hash, Hasher},
    process::Command,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Manager};
use uuid::Uuid;
use walkdir::WalkDir;

static INITIALIZED_DATABASES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
static ACCOUNT_SCOPE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static DISCORD_RPC: OnceLock<Mutex<Option<DiscordIpcClient>>> = OnceLock::new();

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
fn hidden_windows_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[derive(Debug, Clone)]
struct DiscoveredGame {
    id: String,
    title: String,
    exe_path: Option<String>,
    install_path: String,
    source: String,
    source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameRecord {
    id: String,
    title: String,
    exe_path: Option<String>,
    install_path: String,
    source: String,
    source_id: Option<String>,
    favorite: bool,
    cover_path: Option<String>,
    cover_data_url: Option<String>,
    added_at: String,
    last_played: Option<String>,
    total_seconds: i64,
    launch_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    found: usize,
    added: usize,
    updated: usize,
    steam_found: usize,
    epic_found: usize,
    gog_found: usize,
    emulator_found: usize,
    device_found: usize,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtworkRefreshResult {
    attempted: usize,
    updated: usize,
    remaining: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchResult {
    started: bool,
    tracking: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stats {
    game_count: i64,
    favorite_count: i64,
    played_game_count: i64,
    total_seconds: i64,
    launch_count: i64,
    last_7_days_seconds: i64,
    screenshot_count: i64,
    top_game: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Achievement {
    id: String,
    title: String,
    description: String,
    unlocked: bool,
    current: i64,
    target: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotRecord {
    id: i64,
    game_id: String,
    path: String,
    data_url: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutoScreenshotScanResult {
    found: usize,
    imported: usize,
    steam_imported: usize,
    matched_imported: usize,
    skipped_duplicates: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionRecord {
    id: String,
    name: String,
    game_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionMembership {
    collection_id: String,
    game_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveConfig {
    profile_id: String,
    game_id: String,
    save_path: String,
    configured_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveBackupRecord {
    id: String,
    profile_id: String,
    game_id: String,
    backup_path: String,
    created_at: String,
    file_count: i64,
    total_bytes: i64,
    kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileRecord {
    id: String,
    name: String,
    created_at: String,
    last_used_at: String,
    game_count: i64,
    backup_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileSaveFileState {
    profile_id: String,
    game_id: String,
    vault_path: String,
    exists: bool,
    file_count: i64,
    total_bytes: i64,
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudUploadResult {
    backup_id: String,
    profile_id: String,
    game_id: String,
    uploaded_files: usize,
    uploaded_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudSyncAllResult {
    backups: usize,
    uploaded_files: usize,
    uploaded_bytes: u64,
}

fn base_app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve Dusk data directory: {error}"))?;
    fs::create_dir_all(&dir).map_err(|error| format!("Could not create data directory: {error}"))?;
    Ok(dir)
}

fn current_account_scope() -> Option<String> {
    ACCOUNT_SCOPE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = base_app_data_dir(app)?;
    let dir = match current_account_scope() {
        Some(user_id) => base.join("accounts").join(user_id),
        None => base,
    };
    fs::create_dir_all(&dir).map_err(|error| format!("Could not create data directory: {error}"))?;
    Ok(dir)
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("dusk.db"))
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    let connection =
        Connection::open(&path).map_err(|error| format!("Could not open Dusk database: {error}"))?;

    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(|error| format!("Could not configure Dusk database timeout: {error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("Could not configure Dusk database: {error}"))?;

    let initialized = INITIALIZED_DATABASES.get_or_init(|| Mutex::new(HashSet::new()));
    let needs_schema = {
        let guard = initialized
            .lock()
            .map_err(|_| "Could not lock Dusk database initialization state.".to_string())?;
        !guard.contains(&path)
    };

    if needs_schema {
        initialize_schema(&connection)?;
        initialized
            .lock()
            .map_err(|_| "Could not lock Dusk database initialization state.".to_string())?
            .insert(path);
    }

    Ok(connection)
}

fn initialize_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                exe_path TEXT,
                install_path TEXT NOT NULL,
                source TEXT NOT NULL,
                source_id TEXT,
                favorite INTEGER NOT NULL DEFAULT 0,
                hidden INTEGER NOT NULL DEFAULT 0,
                cover_path TEXT,
                added_at TEXT NOT NULL,
                last_played TEXT,
                total_seconds INTEGER NOT NULL DEFAULT 0,
                launch_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS screenshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                source_path TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS collection_games (
                collection_id TEXT NOT NULL,
                game_id TEXT NOT NULL,
                PRIMARY KEY(collection_id, game_id),
                FOREIGN KEY(collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL COLLATE NOCASE,
                created_at TEXT NOT NULL,
                last_used_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS profile_achievements (
                profile_id TEXT NOT NULL,
                achievement_id TEXT NOT NULL,
                current INTEGER NOT NULL DEFAULT 0,
                unlocked INTEGER NOT NULL DEFAULT 0,
                unlocked_at TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(profile_id, achievement_id),
                FOREIGN KEY(profile_id) REFERENCES profiles(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_profile_achievements_profile
            ON profile_achievements(profile_id, achievement_id);

            CREATE TABLE IF NOT EXISTS profile_games (
                profile_id TEXT NOT NULL,
                game_id TEXT NOT NULL,
                added_at TEXT NOT NULL,
                PRIMARY KEY(profile_id, game_id),
                FOREIGN KEY(profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS profile_save_configs (
                profile_id TEXT NOT NULL,
                game_id TEXT NOT NULL,
                save_path TEXT NOT NULL,
                configured_at TEXT NOT NULL,
                PRIMARY KEY(profile_id, game_id),
                FOREIGN KEY(profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS profile_save_backups (
                id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL,
                game_id TEXT NOT NULL,
                backup_path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                file_count INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                kind TEXT NOT NULL DEFAULT 'manual',
                FOREIGN KEY(profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_profile_games_profile
            ON profile_games(profile_id, game_id);

            CREATE INDEX IF NOT EXISTS idx_profile_save_backups_profile_game
            ON profile_save_backups(profile_id, game_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS save_configs (
                game_id TEXT PRIMARY KEY,
                save_path TEXT NOT NULL,
                configured_at TEXT NOT NULL,
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS save_backups (
                id TEXT PRIMARY KEY,
                game_id TEXT NOT NULL,
                backup_path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                file_count INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                kind TEXT NOT NULL DEFAULT 'manual',
                FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_save_backups_game_created
            ON save_backups(game_id, created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_games_last_played ON games(last_played);
            CREATE INDEX IF NOT EXISTS idx_sessions_game_id ON sessions(game_id);
            CREATE INDEX IF NOT EXISTS idx_screenshots_game_id ON screenshots(game_id);
            "#,
        )
        .map_err(|error| format!("Could not initialize Dusk database: {error}"))?;

    connection
        .execute_batch(
            r#"
            INSERT OR IGNORE INTO profiles (id, name, created_at, last_used_at)
            VALUES ('default', 'Player 1', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT OR IGNORE INTO app_settings (key, value)
            VALUES ('active_profile_id', 'default');
            "#,
        )
        .map_err(|error| format!("Could not initialize Dusk profiles: {error}"))?;

    let profiles_migrated: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'profiles_v1_migrated' AND value = '1')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect profile migration state: {error}"))?;

    if !profiles_migrated {
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| format!("Could not begin profile migration: {error}"))?;

        transaction
            .execute_batch(
                r#"
                INSERT OR IGNORE INTO profile_games (profile_id, game_id, added_at)
                SELECT 'default', id, added_at FROM games;

                INSERT OR IGNORE INTO profile_save_configs
                    (profile_id, game_id, save_path, configured_at)
                SELECT 'default', game_id, save_path, configured_at
                FROM save_configs;

                INSERT OR IGNORE INTO profile_save_backups
                    (id, profile_id, game_id, backup_path, created_at, file_count, total_bytes, kind)
                SELECT id, 'default', game_id, backup_path, created_at, file_count, total_bytes, kind
                FROM save_backups;

                INSERT OR REPLACE INTO app_settings (key, value)
                VALUES ('profiles_v1_migrated', '1');
                "#,
            )
            .map_err(|error| format!("Could not migrate legacy Dusk data into profiles: {error}"))?;

        transaction
            .commit()
            .map_err(|error| format!("Could not commit profile migration: {error}"))?;
    }

    // Existing Dusk databases predate automatic screenshot source tracking.
    let _ = connection.execute("ALTER TABLE screenshots ADD COLUMN source_path TEXT", []);
    let _ = connection.execute("ALTER TABLE games ADD COLUMN cover_origin TEXT", []);
    connection
        .execute_batch(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_screenshots_source_path
            ON screenshots(source_path)
            WHERE source_path IS NOT NULL;

            CREATE TABLE IF NOT EXISTS ignored_screenshot_sources (
                source_path TEXT PRIMARY KEY,
                ignored_at TEXT NOT NULL
            );
            "#,
        )
        .map_err(|error| format!("Could not initialize screenshot tracking: {error}"))
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn active_profile_id(connection: &Connection) -> Result<String, String> {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'active_profile_id'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not load active profile: {error}"))
}

fn attach_game_to_active_profile(connection: &Connection, game_id: &str) -> Result<(), String> {
    let profile_id = active_profile_id(connection)?;
    connection
        .execute(
            "INSERT OR IGNORE INTO profile_games (profile_id, game_id, added_at) VALUES (?1, ?2, ?3)",
            params![profile_id, game_id, now()],
        )
        .map_err(|error| format!("Could not attach game to profile: {error}"))?;
    Ok(())
}

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileRecord> {
    Ok(ProfileRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        last_used_at: row.get(3)?,
        game_count: row.get(4)?,
        backup_count: row.get(5)?,
    })
}

fn row_to_game(row: &rusqlite::Row<'_>) -> rusqlite::Result<GameRecord> {
    let cover_path: Option<String> = row.get(7)?;

    Ok(GameRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        exe_path: row.get(2)?,
        install_path: row.get(3)?,
        source: row.get(4)?,
        source_id: row.get(5)?,
        favorite: row.get::<_, i64>(6)? != 0,
        cover_path,
        cover_data_url: None,
        added_at: row.get(8)?,
        last_played: row.get(9)?,
        total_seconds: row.get(10)?,
        launch_count: row.get(11)?,
    })
}

fn get_game(connection: &Connection, id: &str) -> Result<GameRecord, String> {
    connection
        .query_row(
            r#"
            SELECT id, title, exe_path, install_path, source, source_id, favorite,
                   cover_path, added_at, last_played, total_seconds, launch_count
            FROM games
            WHERE id = ?1 AND hidden = 0
            "#,
            params![id],
            row_to_game,
        )
        .optional()
        .map_err(|error| format!("Could not read game: {error}"))?
        .ok_or_else(|| "Game not found.".to_string())
}

fn upsert_discovered(connection: &Connection, game: &DiscoveredGame) -> Result<bool, String> {
    let existed: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
            params![game.id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check game: {error}"))?;

    connection
        .execute(
            r#"
            INSERT INTO games (
                id, title, exe_path, install_path, source, source_id, added_at, hidden
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                exe_path = COALESCE(excluded.exe_path, games.exe_path),
                install_path = excluded.install_path,
                source = excluded.source,
                source_id = excluded.source_id,
                hidden = 0
            "#,
            params![
                game.id,
                game.title,
                game.exe_path,
                game.install_path,
                game.source,
                game.source_id,
                now()
            ],
        )
        .map_err(|error| format!("Could not save discovered game: {error}"))?;

    attach_game_to_active_profile(connection, &game.id)?;
    Ok(existed)
}

fn quoted_vdf_value(contents: &str, key: &str) -> Option<String> {
    let pattern = format!(r#""{}"\s+"([^"]+)""#, regex::escape(key));
    Regex::new(&pattern)
        .ok()?
        .captures(contents)?
        .get(1)
        .map(|value| value.as_str().replace(r"\\", r"\"))
}

fn find_local_artwork(root: &Path) -> Option<PathBuf> {
    if !root.exists() { return None; }
    // Portrait-first artwork only. Wide headers/heroes/banners crop badly in Dusk's cards.
    let preferred = ["library_600x900", "cover", "poster", "keyart", "capsule", "boxart", "vertical"];
    let mut best: Option<(i64, PathBuf)> = None;
    for entry in WalkDir::new(root).max_depth(4).follow_links(false).into_iter().filter_map(Result::ok).take(3000) {
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        let ext = path.extension().and_then(|v| v.to_str()).unwrap_or_default().to_ascii_lowercase();
        if !["png","jpg","jpeg","webp"].contains(&ext.as_str()) { continue; }
        let name = path.file_stem().and_then(|v| v.to_str()).unwrap_or_default().to_ascii_lowercase();
        let mut score = 40_i64 - entry.depth() as i64 * 4;
        for (index, word) in preferred.iter().enumerate() {
            if name.contains(word) { score += 120 - index as i64 * 8; }
        }
        if name.contains("header") || name.contains("hero") || name.contains("banner") { score -= 160; }
        if name.contains("icon") || name.contains("logo") { score -= 80; }
        if best.as_ref().map(|(current, _)| score > *current).unwrap_or(true) {
            best = Some((score, path.to_path_buf()));
        }
    }
    best.filter(|(score, _)| *score >= 80).map(|(_, path)| path)
}


fn normalized_artwork_match_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_match_confidence(expected: &str, candidate: &str) -> f32 {
    let expected = normalized_artwork_match_text(expected);
    let candidate = normalized_artwork_match_text(candidate);
    if expected.is_empty() || candidate.is_empty() {
        return 0.0;
    }
    if expected == candidate {
        return 1.0;
    }
    if candidate.contains(&expected) || expected.contains(&candidate) {
        return 0.92;
    }
    let expected_tokens: HashSet<&str> = expected.split_whitespace().collect();
    let candidate_tokens: HashSet<&str> = candidate.split_whitespace().collect();
    let common = expected_tokens.intersection(&candidate_tokens).count() as f32;
    let denom = expected_tokens.len().max(candidate_tokens.len()) as f32;
    if denom <= 0.0 { 0.0 } else { common / denom }
}

#[cfg(target_os = "windows")]
fn executable_company_name(exe_path: Option<&str>) -> Option<String> {
    let path = exe_path?;
    if !Path::new(path).is_file() {
        return None;
    }
    let escaped = path.replace('\'', "''");
    let script = format!(
        "$v=(Get-Item -LiteralPath '{}').VersionInfo.CompanyName; if($v){{$v}}",
        escaped
    );
    let output = hidden_windows_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(not(target_os = "windows"))]
fn executable_company_name(_exe_path: Option<&str>) -> Option<String> {
    None
}

fn wikimedia_image_for_filename(
    client: &reqwest::blocking::Client,
    filename: &str,
) -> Option<String> {
    let title = format!("File:{filename}");
    let response = client
        .get("https://commons.wikimedia.org/w/api.php")
        .query(&[
            ("action", "query"),
            ("titles", title.as_str()),
            ("prop", "imageinfo"),
            ("iiprop", "url|size"),
            ("iiurlwidth", "900"),
            ("format", "json"),
            ("formatversion", "2"),
        ])
        .send()
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().ok()?;
    let page = json.get("query")?.get("pages")?.as_array()?.first()?;
    let info = page.get("imageinfo")?.as_array()?.first()?;
    let width = info.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
    let height = info.get("height").and_then(|v| v.as_u64()).unwrap_or(0);

    // Dusk cards are portrait; reject square/landscape media.
    if width == 0 || height == 0 || height < width.saturating_mul(6) / 5 {
        return None;
    }

    info.get("thumburl")
        .or_else(|| info.get("url"))
        .and_then(|v| v.as_str())
        .filter(|url| url.starts_with("https://"))
        .map(ToOwned::to_owned)
}

fn wikidata_entity_details(
    client: &reqwest::blocking::Client,
    entity_id: &str,
) -> Option<serde_json::Value> {
    let response = client
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbgetentities"),
            ("ids", entity_id),
            ("props", "claims|labels|descriptions"),
            ("languages", "en"),
            ("format", "json"),
        ])
        .send()
        .ok()?;

    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().ok()?;
    json.get("entities")?.get(entity_id).cloned()
}

fn wikidata_related_labels(
    client: &reqwest::blocking::Client,
    entity: &serde_json::Value,
) -> Vec<String> {
    let mut ids = Vec::new();
    for property in ["P123", "P178"] {
        if let Some(claims) = entity
            .get("claims")
            .and_then(|v| v.get(property))
            .and_then(|v| v.as_array())
        {
            for claim in claims {
                if let Some(id) = claim
                    .get("mainsnak")
                    .and_then(|v| v.get("datavalue"))
                    .and_then(|v| v.get("value"))
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Vec::new();
    }

    let joined = ids.join("|");
    let response = match client
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbgetentities"),
            ("ids", joined.as_str()),
            ("props", "labels"),
            ("languages", "en"),
            ("format", "json"),
        ])
        .send()
    {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };

    let json: serde_json::Value = match response.json() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    ids.into_iter()
        .filter_map(|id| {
            json.get("entities")?
                .get(&id)?
                .get("labels")?
                .get("en")?
                .get("value")?
                .as_str()
                .map(ToOwned::to_owned)
        })
        .collect()
}


fn download_cover_url(
    client: &reqwest::blocking::Client,
    url: &str,
    covers_dir: &Path,
    base: &str,
) -> Option<PathBuf> {
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    let extension = if content_type.contains("png") {
        "png"
    } else if content_type.contains("webp") {
        "webp"
    } else {
        "jpg"
    };

    let bytes = response.bytes().ok()?;
    if bytes.len() < 4096 {
        return None;
    }

    let destination = covers_dir.join(format!("{base}.{extension}"));
    fs::write(&destination, &bytes).ok()?;
    Some(destination)
}


#[derive(Debug, Clone)]
struct CoverAiCandidate {
    title: String,
    publisher: Option<String>,
    source: &'static str,
    image_url: String,
    query_title: String,
    portrait_hint: bool,
}

fn cover_ai_title_variants(title: &str) -> Vec<String> {
    let original = title.trim();
    if original.is_empty() {
        return Vec::new();
    }

    let mut variants = vec![original.to_string()];
    let mut words: Vec<&str> = original.split_whitespace().collect();
    let removable_suffixes = [
        "demo", "playtest", "beta", "alpha", "test", "trial", "prototype", "preview", "og",
    ];

    while words.len() > 1 {
        let last = words
            .last()
            .map(|value| value.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_ascii_lowercase())
            .unwrap_or_default();
        if !removable_suffixes.contains(&last.as_str()) {
            break;
        }
        words.pop();
    }

    if !words.is_empty() {
        let stripped = words.join(" ");
        if !stripped.eq_ignore_ascii_case(original) {
            variants.push(stripped);
        }
    }

    if let Some(index) = original.rfind(" (") {
        if original.ends_with(')') && index > 0 {
            variants.push(original[..index].trim().to_string());
        }
    }

    variants.retain(|value| !value.trim().is_empty());
    variants.sort_by_key(|value| std::cmp::Reverse(value.len()));
    variants.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    variants
}

fn cover_ai_company_match(expected: Option<&str>, actual: Option<&str>) -> bool {
    let (Some(expected), Some(actual)) = (expected, actual) else {
        return false;
    };
    let expected = normalized_artwork_match_text(expected);
    let actual = normalized_artwork_match_text(actual);
    !expected.is_empty()
        && !actual.is_empty()
        && (expected == actual || expected.contains(&actual) || actual.contains(&expected))
}

fn cover_ai_score(
    game: &DiscoveredGame,
    candidate: &CoverAiCandidate,
    exe_company: Option<&str>,
) -> f32 {
    let direct = title_match_confidence(&game.title, &candidate.title);
    let variant_best = cover_ai_title_variants(&game.title)
        .iter()
        .map(|variant| title_match_confidence(variant, &candidate.title))
        .fold(0.0_f32, f32::max);
    let query_fit = title_match_confidence(&candidate.query_title, &candidate.title);

    let mut score = direct * 0.32 + variant_best * 0.46 + query_fit * 0.10;

    if candidate.source == game.source {
        score += 0.07;
    }
    if cover_ai_company_match(exe_company, candidate.publisher.as_deref()) {
        score += 0.08;
    }
    if candidate.portrait_hint {
        score += 0.05;
    }

    let candidate_normalized = normalized_artwork_match_text(&candidate.title);
    let game_normalized = normalized_artwork_match_text(&game.title);
    if candidate_normalized.contains("soundtrack") && !game_normalized.contains("soundtrack") {
        score -= 0.35;
    }
    if candidate_normalized.contains("dlc") && !game_normalized.contains("dlc") {
        score -= 0.30;
    }

    score.clamp(0.0, 1.0)
}

fn cover_ai_collect_steam_candidates(
    client: &reqwest::blocking::Client,
    query_title: &str,
) -> Vec<CoverAiCandidate> {
    let mut candidates = Vec::new();

    if let Ok(response) = client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[
            ("term", query_title),
            ("l", "english"),
            ("cc", "US"),
        ])
        .send()
    {
        if response.status().is_success() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                if let Some(items) = json.get("items").and_then(|value| value.as_array()) {
                    for item in items.iter().take(8) {
                        let Some(title) = item.get("name").and_then(|value| value.as_str()) else { continue };
                        let app_id = item
                            .get("id")
                            .and_then(|value| value.as_u64())
                            .or_else(|| item.get("id").and_then(|value| value.as_str()).and_then(|value| value.parse().ok()));
                        let Some(app_id) = app_id else { continue };
                        candidates.push(CoverAiCandidate {
                            title: title.to_string(),
                            publisher: None,
                            source: "steam",
                            image_url: format!(
                                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg"
                            ),
                            query_title: query_title.to_string(),
                            portrait_hint: true,
                        });
                    }
                }
            }
        }
    }

    // Secondary Steam name search. This often finds apps the Store search API omits.
    if let Ok(mut url) = reqwest::Url::parse("https://steamcommunity.com/actions/SearchApps/") {
        if let Ok(mut segments) = url.path_segments_mut() {
            segments.push(query_title);
        }
        if let Ok(response) = client.get(url).send() {
            if response.status().is_success() {
                if let Ok(items) = response.json::<Vec<serde_json::Value>>() {
                    for item in items.into_iter().take(8) {
                        let Some(title) = item.get("name").and_then(|value| value.as_str()) else { continue };
                        let app_id = item
                            .get("appid")
                            .and_then(|value| value.as_u64())
                            .or_else(|| item.get("appid").and_then(|value| value.as_str()).and_then(|value| value.parse().ok()));
                        let Some(app_id) = app_id else { continue };
                        candidates.push(CoverAiCandidate {
                            title: title.to_string(),
                            publisher: None,
                            source: "steam",
                            image_url: format!(
                                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg"
                            ),
                            query_title: query_title.to_string(),
                            portrait_hint: true,
                        });
                    }
                }
            }
        }
    }

    candidates
}

fn cover_ai_collect_epic_candidates(
    client: &reqwest::blocking::Client,
    query_title: &str,
) -> Vec<CoverAiCandidate> {
    let variables = serde_json::json!({
        "category": "games/edition/base|bundles/games|games/edition|games/demo|games/experience",
        "count": 20,
        "start": 0,
        "sortBy": "relevancy",
        "sortDir": "DESC",
        "keywords": query_title,
        "allowCountries": "US",
        "comingSoon": false,
        "withPrice": false,
        "country": "US",
        "locale": "en-US"
    })
    .to_string();

    let extensions = serde_json::json!({
        "persistedQuery": {
            "version": 1,
            "sha256Hash": "7d58e12d9dd8cb14c84a3ff18d360bf9f0caa96bf218f2c5fda68ba88d68a437"
        }
    })
    .to_string();

    let response = match client
        .get("https://store.epicgames.com/graphql")
        .query(&[
            ("operationName", "searchStoreQuery"),
            ("variables", variables.as_str()),
            ("extensions", extensions.as_str()),
        ])
        .send()
    {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };

    let json: serde_json::Value = match response.json() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let Some(elements) = json
        .get("data")
        .and_then(|value| value.get("Catalog"))
        .and_then(|value| value.get("searchStore"))
        .and_then(|value| value.get("elements"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let priorities = [
        "DieselGameBoxTall",
        "OfferImageTall",
        "DieselStoreFrontTall",
        "VaultClosed",
        "Thumbnail",
    ];

    let mut candidates = Vec::new();
    for element in elements.iter().take(12) {
        let Some(title) = element.get("title").and_then(|value| value.as_str()) else { continue };
        let publisher = element
            .get("seller")
            .and_then(|value| value.get("name"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);

        let Some(images) = element.get("keyImages").and_then(|value| value.as_array()) else { continue };
        let mut chosen = None;
        for wanted in priorities {
            if let Some(url) = images.iter().find_map(|image| {
                let kind = image.get("type").and_then(|value| value.as_str())?;
                let url = image.get("url").and_then(|value| value.as_str())?;
                if kind.eq_ignore_ascii_case(wanted) && url.starts_with("https://") {
                    Some(url.to_string())
                } else {
                    None
                }
            }) {
                chosen = Some(url);
                break;
            }
        }

        let Some(image_url) = chosen else { continue };
        candidates.push(CoverAiCandidate {
            title: title.to_string(),
            publisher,
            source: "epic",
            image_url,
            query_title: query_title.to_string(),
            portrait_hint: true,
        });
    }

    candidates
}

fn cover_ai_web_fallback(
    game: &DiscoveredGame,
    covers_dir: &Path,
    base: &str,
) -> Option<PathBuf> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Dusk-Cover-AI/1.7.8")
        .build()
        .ok()?;

    let company = executable_company_name(game.exe_path.as_deref());
    let mut candidates = Vec::new();

    for query in cover_ai_title_variants(&game.title).into_iter().take(3) {
        candidates.extend(cover_ai_collect_steam_candidates(&client, &query));
        candidates.extend(cover_ai_collect_epic_candidates(&client, &query));
    }

    let mut scored: Vec<(f32, CoverAiCandidate)> = candidates
        .into_iter()
        .map(|candidate| {
            let score = cover_ai_score(game, &candidate, company.as_deref());
            (score, candidate)
        })
        .filter(|(score, _)| *score >= 0.70)
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut seen_urls = HashSet::new();
    for (_score, candidate) in scored.into_iter().take(10) {
        if !seen_urls.insert(candidate.image_url.clone()) {
            continue;
        }
        if let Some(path) = download_cover_url(&client, &candidate.image_url, covers_dir, base) {
            return Some(path);
        }
    }

    None
}

fn epic_store_cover_fallback(
    game: &DiscoveredGame,
    covers_dir: &Path,
    base: &str,
) -> Option<PathBuf> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Dusk-Desktop-Artwork/1.7.6")
        .build()
        .ok()?;

    let graphql = r#"
      query DuskSearch($keywords: String!) {
        Catalog {
          searchStore(
            country: "US"
            locale: "en-US"
            keywords: $keywords
            count: 10
            start: 0
          ) {
            elements {
              title
              developerDisplayName
              publisherDisplayName
              seller { name }
              keyImages { type url }
            }
          }
        }
      }
    "#;

    let payload = serde_json::json!({
        "query": graphql,
        "variables": { "keywords": game.title }
    });

    let response = client
        .post("https://graphql.epicgames.com/graphql")
        .json(&payload)
        .send()
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().ok()?;
    let elements = json
        .get("data")?
        .get("Catalog")?
        .get("searchStore")?
        .get("elements")?
        .as_array()?;

    let exe_company = executable_company_name(game.exe_path.as_deref())
        .map(|value| normalized_artwork_match_text(&value));

    let mut best: Option<(f32, String)> = None;

    for element in elements {
        let title = element.get("title").and_then(|v| v.as_str()).unwrap_or_default();
        let title_score = title_match_confidence(&game.title, title);
        if title_score < 0.82 {
            continue;
        }

        let mut company_match = false;
        if let Some(expected) = exe_company.as_deref() {
            for field in ["developerDisplayName", "publisherDisplayName"] {
                if let Some(value) = element.get(field).and_then(|v| v.as_str()) {
                    let actual = normalized_artwork_match_text(value);
                    if actual == expected || actual.contains(expected) || expected.contains(&actual) {
                        company_match = true;
                    }
                }
            }
            if let Some(value) = element
                .get("seller")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
            {
                let actual = normalized_artwork_match_text(value);
                if actual == expected || actual.contains(expected) || expected.contains(&actual) {
                    company_match = true;
                }
            }
        }

        if title_score < 0.93 && exe_company.is_some() && !company_match {
            continue;
        }

        let images = match element.get("keyImages").and_then(|v| v.as_array()) {
            Some(value) => value,
            None => continue,
        };

        let priorities = [
            "DieselGameBoxTall",
            "OfferImageTall",
            "DieselStoreFrontTall",
            "VaultClosed",
            "Thumbnail",
        ];

        let mut chosen: Option<String> = None;
        for wanted in priorities {
            if let Some(url) = images.iter().find_map(|image| {
                let kind = image.get("type").and_then(|v| v.as_str())?;
                let url = image.get("url").and_then(|v| v.as_str())?;
                if kind.eq_ignore_ascii_case(wanted) && url.starts_with("https://") {
                    Some(url.to_string())
                } else {
                    None
                }
            }) {
                chosen = Some(url);
                break;
            }
        }

        let Some(url) = chosen else { continue };
        let score = title_score + if company_match { 0.07 } else { 0.0 };
        if best.as_ref().map(|(current, _)| score > *current).unwrap_or(true) {
            best = Some((score, url));
        }
    }

    let (score, url) = best?;
    if score < 0.9 {
        return None;
    }
    download_cover_url(&client, &url, covers_dir, base)
}

fn steam_store_cover_fallback(
    game: &DiscoveredGame,
    covers_dir: &Path,
    base: &str,
) -> Option<PathBuf> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(7))
        .user_agent("Dusk-Desktop-Artwork/1.7.6")
        .build()
        .ok()?;

    let response = client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[
            ("term", game.title.as_str()),
            ("l", "english"),
            ("cc", "US"),
        ])
        .send()
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().ok()?;
    let items = json.get("items")?.as_array()?;

    let mut candidates: Vec<(f32, u64)> = items
        .iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?;
            let id = item.get("id")?.as_u64()?;
            let score = title_match_confidence(&game.title, name);
            if score >= 0.9 { Some((score, id)) } else { None }
        })
        .collect();

    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    for (_, app_id) in candidates.into_iter().take(4) {
        for extension in ["jpg", "png"] {
            let url = format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.{extension}"
            );
            if let Some(path) = download_cover_url(&client, &url, covers_dir, base) {
                return Some(path);
            }
        }
    }

    None
}

fn wikidata_cover_fallback(
    game: &DiscoveredGame,
    covers_dir: &Path,
    base: &str,
) -> Option<PathBuf> {
    // Final fallback only: manual, launcher-native and local portrait artwork
    // are checked before this function is called.
    let publisher = executable_company_name(game.exe_path.as_deref());
    let normalized_publisher = publisher.as_deref().map(normalized_artwork_match_text);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(7))
        .user_agent("Dusk-Desktop-Artwork/1.7.6")
        .build()
        .ok()?;

    let response = client
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbsearchentities"),
            ("search", game.title.as_str()),
            ("language", "en"),
            ("type", "item"),
            ("limit", "8"),
            ("format", "json"),
        ])
        .send()
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().ok()?;
    let results = json.get("search")?.as_array()?;
    let mut candidates: Vec<(f32, String)> = Vec::new();

    for result in results {
        let id = match result.get("id").and_then(|v| v.as_str()) {
            Some(value) => value,
            None => continue,
        };
        let label = result.get("label").and_then(|v| v.as_str()).unwrap_or_default();
        let description = result
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let description_normalized = normalized_artwork_match_text(description);
        if !description_normalized.contains("video game")
            && !description_normalized.contains("videogame")
        {
            continue;
        }

        let title_score = title_match_confidence(&game.title, label);
        if title_score < 0.72 {
            continue;
        }

        let entity = match wikidata_entity_details(&client, id) {
            Some(value) => value,
            None => continue,
        };

        let related_labels = wikidata_related_labels(&client, &entity);
        let publisher_match = normalized_publisher
            .as_deref()
            .map(|expected| {
                related_labels.iter().any(|label| {
                    let actual = normalized_artwork_match_text(label);
                    actual == expected || actual.contains(expected) || expected.contains(&actual)
                })
            })
            .unwrap_or(false);

        // Exact/near-exact title is allowed without publisher metadata.
        // Weaker title matches require publisher/developer agreement.
        if title_score < 0.9 && normalized_publisher.is_some() && !publisher_match {
            continue;
        }

        let filename = entity
            .get("claims")
            .and_then(|v| v.get("P18"))
            .and_then(|v| v.as_array())
            .and_then(|values| values.first())
            .and_then(|claim| claim.get("mainsnak"))
            .and_then(|v| v.get("datavalue"))
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str());

        let Some(filename) = filename else {
            continue;
        };

        if wikimedia_image_for_filename(&client, filename).is_none() {
            continue;
        }

        let score = title_score + if publisher_match { 0.08 } else { 0.0 };
        candidates.push((score, id.to_string()));
    }

    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    for (score, id) in candidates {
        if score < 0.82 {
            continue;
        }
        let entity = wikidata_entity_details(&client, &id)?;
        let filename = entity
            .get("claims")
            .and_then(|v| v.get("P18"))
            .and_then(|v| v.as_array())
            .and_then(|values| values.first())
            .and_then(|claim| claim.get("mainsnak"))
            .and_then(|v| v.get("datavalue"))
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())?;

        let image_url = match wikimedia_image_for_filename(&client, filename) {
            Some(value) => value,
            None => continue,
        };

        let image_response = match client.get(&image_url).send() {
            Ok(response) if response.status().is_success() => response,
            _ => continue,
        };

        let content_type = image_response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let extension = if content_type.contains("png") {
            "png"
        } else if content_type.contains("webp") {
            "webp"
        } else {
            "jpg"
        };

        let bytes = match image_response.bytes() {
            Ok(value) if value.len() >= 4096 => value,
            _ => continue,
        };

        let destination = covers_dir.join(format!("{base}.{extension}"));
        if fs::write(&destination, &bytes).is_ok() {
            return Some(destination);
        }
    }

    None
}

fn auto_apply_game_artwork(app: &AppHandle, connection: &Connection, game: &DiscoveredGame) {
    let existing: Option<(Option<String>, Option<String>)> = connection.query_row(
        "SELECT cover_path, cover_origin FROM games WHERE id = ?1",
        params![game.id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().ok().flatten();

    if let Some((Some(existing_path), Some(origin))) = existing.as_ref() {
        if origin == "manual" && Path::new(existing_path).exists() { return; }
    }

    let covers_dir = match app_data_dir(app) { Ok(dir) => dir.join("covers"), Err(_) => return };
    if fs::create_dir_all(&covers_dir).is_err() { return; }
    let base = game.id.replace(':', "_");

    // 1) Launcher-native local Steam library artwork.
    if game.source == "steam" {
        if let Some(app_id) = game.source_id.as_deref() {
            for steam_root in steam_roots() {
                let cache = steam_root.join("appcache").join("librarycache");
                for filename in [
                    format!("{app_id}_library_600x900.jpg"),
                    format!("{app_id}_library_600x900.png"),
                    format!("{app_id}_library_capsule.jpg"),
                    format!("{app_id}_library_capsule.png"),
                ] {
                    let source = cache.join(filename);
                    if source.is_file() {
                        let ext = source.extension().and_then(|v| v.to_str()).unwrap_or("jpg");
                        let destination = covers_dir.join(format!("{base}.{ext}"));
                        if fs::copy(&source, &destination).is_ok() {
                            let _ = connection.execute(
                                "UPDATE games SET cover_path = ?1, cover_origin = 'auto' WHERE id = ?2",
                                params![destination.to_string_lossy().into_owned(), game.id],
                            );
                            return;
                        }
                    }
                }
            }
        }
    }

    // 2) Local portrait cover/poster/key art from the installed game.
    if let Some(source) = find_local_artwork(Path::new(&game.install_path)) {
        let ext = source.extension().and_then(|v| v.to_str()).unwrap_or("png").to_ascii_lowercase();
        let destination = covers_dir.join(format!("{base}.{ext}"));
        if fs::copy(&source, &destination).is_ok() {
            let _ = connection.execute(
                "UPDATE games SET cover_path = ?1, cover_origin = 'auto' WHERE id = ?2",
                params![destination.to_string_lossy().into_owned(), game.id],
            );
            return;
        }
    }

    // 3) Small local Cover AI: search/rank official store candidates only as fallback.
    if let Some(destination) = cover_ai_web_fallback(game, &covers_dir, &base) {
        let _ = connection.execute(
            "UPDATE games SET cover_path = ?1, cover_origin = 'ai-web' WHERE id = ?2",
            params![destination.to_string_lossy().into_owned(), game.id],
        );
        return;
    }

    // 4) Structured knowledge fallback if store candidate search finds nothing.
    if let Some(destination) = wikidata_cover_fallback(game, &covers_dir, &base) {
        let _ = connection.execute(
            "UPDATE games SET cover_path = ?1, cover_origin = 'web-fallback' WHERE id = ?2",
            params![destination.to_string_lossy().into_owned(), game.id],
        );
    }

}

fn find_best_executable(root: &Path, game_name: &str) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }

    let normalized_name: String = game_name
        .to_ascii_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();

    let mut candidates: Vec<(i64, PathBuf)> = Vec::new();

    for entry in WalkDir::new(root)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()).map(|value| value.eq_ignore_ascii_case("exe")) != Some(true) {
            continue;
        }

        let filename = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let blocked = [
            "unins",
            "uninstall",
            "crash",
            "report",
            "vc_redist",
            "vcredist",
            "dxsetup",
            "setup",
            "unitycrashhandler",
            "easyanticheat",
            "eac",
            "battleye",
            "redist",
            "prereq",
            "bootstrapper",
            "updater",
            "update",
            "helper",
            "service",
            "cefprocess",
            "dotnet",
        ];
        if blocked.iter().any(|word| filename.contains(word)) {
            continue;
        }

        let normalized_file: String = filename
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect();

        let mut score = 100_i64 - entry.depth() as i64 * 8;
        if !normalized_name.is_empty()
            && (normalized_file.contains(&normalized_name)
                || normalized_name.contains(&normalized_file))
        {
            score += 150;
        }
        if filename.contains("launcher") {
            score -= 20;
        }
        if filename.contains("win64") || filename.contains("x64") {
            score += 10;
        }

        candidates.push((score, path.to_path_buf()));
    }

    candidates
        .into_iter()
        .max_by_key(|(score, _)| *score)
        .map(|(_, path)| path)
}


fn find_best_executable_bounded(
    root: &Path,
    game_name: &str,
    started: Instant,
    budget: Duration,
) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }

    let normalized_name: String = game_name
        .to_ascii_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();

    let mut best: Option<(i64, PathBuf)> = None;

    for entry in WalkDir::new(root)
        .max_depth(4)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .take(2000)
    {
        if started.elapsed() >= budget {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("exe"))
            != Some(true)
        {
            continue;
        }

        let filename = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let blocked = [
            "unins",
            "uninstall",
            "crash",
            "report",
            "vc_redist",
            "vcredist",
            "dxsetup",
            "setup",
            "unitycrashhandler",
            "dotnet",
        ];
        if blocked.iter().any(|word| filename.contains(word)) {
            continue;
        }

        let normalized_file: String = filename
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect();

        let mut score = 100_i64 - entry.depth() as i64 * 8;
        if !normalized_name.is_empty()
            && (normalized_file.contains(&normalized_name)
                || normalized_name.contains(&normalized_file))
        {
            score += 150;
        }
        if filename.contains("launcher") {
            score -= 20;
        }
        if filename.contains("win64") || filename.contains("x64") {
            score += 10;
        }

        if best.as_ref().map(|(current, _)| score > *current).unwrap_or(true) {
            best = Some((score, path.to_path_buf()));
        }
    }

    best.map(|(_, path)| path)
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(program_files_x86) = env::var("PROGRAMFILES(X86)") {
        roots.push(PathBuf::from(program_files_x86).join("Steam"));
    }
    if let Ok(program_files) = env::var("PROGRAMFILES") {
        roots.push(PathBuf::from(program_files).join("Steam"));
    }
    roots.sort();
    roots.dedup();
    roots.into_iter().filter(|path| path.exists()).collect()
}

fn scan_steam() -> (Vec<DiscoveredGame>, Vec<String>) {
    let mut games = Vec::new();
    let mut warnings = Vec::new();
    let mut libraries: HashSet<PathBuf> = HashSet::new();

    for root in steam_roots() {
        libraries.insert(root.clone());

        let library_file = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(contents) = fs::read_to_string(&library_file) {
            if let Ok(regex) = Regex::new(r#""path"\s+"([^"]+)""#) {
                for captures in regex.captures_iter(&contents) {
                    if let Some(value) = captures.get(1) {
                        let decoded = value.as_str().replace(r"\\", r"\");
                        libraries.insert(PathBuf::from(decoded));
                    }
                }
            }
        }
    }

    if libraries.is_empty() {
        warnings.push("Steam installation was not found in the standard Windows locations.".into());
        return (games, warnings);
    }

    let mut seen_ids = HashSet::new();

    for library in libraries {
        let steamapps = library.join("steamapps");
        let entries = match fs::read_dir(&steamapps) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let filename = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();

            if !filename.starts_with("appmanifest_") || !filename.ends_with(".acf") {
                continue;
            }

            let contents = match fs::read_to_string(&path) {
                Ok(contents) => contents,
                Err(_) => continue,
            };

            let app_id = quoted_vdf_value(&contents, "appid").or_else(|| {
                filename
                    .strip_prefix("appmanifest_")
                    .and_then(|value| value.strip_suffix(".acf"))
                    .map(ToOwned::to_owned)
            });
            let title = quoted_vdf_value(&contents, "name");
            let install_dir = quoted_vdf_value(&contents, "installdir");

            let (app_id, title, install_dir) = match (app_id, title, install_dir) {
                (Some(app_id), Some(title), Some(install_dir)) => (app_id, title, install_dir),
                _ => continue,
            };

            if !seen_ids.insert(app_id.clone()) {
                continue;
            }

            let install_path = steamapps.join("common").join(&install_dir);
            let exe_path = find_best_executable(&install_path, &title)
                .map(|value| value.to_string_lossy().into_owned());

            games.push(DiscoveredGame {
                id: format!("steam:{app_id}"),
                title,
                exe_path,
                install_path: install_path.to_string_lossy().into_owned(),
                source: "steam".into(),
                source_id: Some(app_id),
            });
        }
    }

    (games, warnings)
}

fn scan_epic() -> (Vec<DiscoveredGame>, Vec<String>) {
    let mut games = Vec::new();
    let mut warnings = Vec::new();

    let program_data = match env::var("PROGRAMDATA") {
        Ok(value) => PathBuf::from(value),
        Err(_) => {
            warnings.push("PROGRAMDATA is unavailable, so Epic games could not be scanned.".into());
            return (games, warnings);
        }
    };

    let manifests = program_data
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");

    if !manifests.exists() {
        return (games, warnings);
    }

    let entries = match fs::read_dir(&manifests) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("Could not read Epic manifests: {error}"));
            return (games, warnings);
        }
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("item") {
            continue;
        }

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&contents) {
            Ok(json) => json,
            Err(_) => continue,
        };

        let title = match json.get("DisplayName").and_then(|value| value.as_str()) {
            Some(value) if !value.trim().is_empty() => value.trim().to_string(),
            _ => continue,
        };
        let install_location = match json.get("InstallLocation").and_then(|value| value.as_str()) {
            Some(value) if !value.trim().is_empty() => PathBuf::from(value),
            _ => continue,
        };

        let source_id = json
            .get("CatalogItemId")
            .and_then(|value| value.as_str())
            .or_else(|| json.get("AppName").and_then(|value| value.as_str()))
            .unwrap_or(&title)
            .to_string();

        let launch_executable = json
            .get("LaunchExecutable")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty());

        let exe = launch_executable
            .map(|value| install_location.join(value))
            .filter(|value| value.exists())
            .or_else(|| find_best_executable(&install_location, &title));

        games.push(DiscoveredGame {
            id: format!("epic:{source_id}"),
            title,
            exe_path: exe.map(|value| value.to_string_lossy().into_owned()),
            install_path: install_location.to_string_lossy().into_owned(),
            source: "epic".into(),
            source_id: Some(source_id),
        });
    }

    (games, warnings)
}


#[cfg(target_os = "windows")]
fn parse_gog_registry_output(output: &str) -> Vec<DiscoveredGame> {
    let mut games = Vec::new();
    let mut current_key = String::new();
    let mut values: HashMap<String, String> = HashMap::new();

    let flush = |key: &str, values: &mut HashMap<String, String>, games: &mut Vec<DiscoveredGame>| {
        if key.is_empty() {
            values.clear();
            return;
        }

        let title = values
            .get("gamename")
            .or_else(|| values.get("displayname"))
            .cloned();

        let install_path = values
            .get("path")
            .or_else(|| values.get("installlocation"))
            .cloned();

        let Some(title) = title else {
            values.clear();
            return;
        };
        let Some(install_path) = install_path else {
            values.clear();
            return;
        };

        let install = PathBuf::from(&install_path);
        if !install.exists() {
            values.clear();
            return;
        }

        let source_id = values
            .get("gameid")
            .cloned()
            .or_else(|| key.rsplit('\\').next().map(ToOwned::to_owned))
            .unwrap_or_else(|| title.clone());

        let exe_path = values
            .get("exe")
            .map(PathBuf::from)
            .filter(|value| value.exists())
            .or_else(|| find_best_executable(&install, &title))
            .map(|value| value.to_string_lossy().into_owned());

        games.push(DiscoveredGame {
            id: format!("gog:{source_id}"),
            title,
            exe_path,
            install_path: install.to_string_lossy().into_owned(),
            source: "gog".into(),
            source_id: Some(source_id),
        });

        values.clear();
    };

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("HKEY_") {
            flush(&current_key, &mut values, &mut games);
            current_key = line.to_string();
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let name = parts[0].to_ascii_lowercase();
        let value = parts[2..].join(" ");
        values.insert(name, value);
    }

    flush(&current_key, &mut values, &mut games);
    games
}

#[cfg(target_os = "windows")]
fn scan_gog() -> (Vec<DiscoveredGame>, Vec<String>) {
    let roots = [
        r"HKLM\SOFTWARE\WOW6432Node\GOG.com\Games",
        r"HKLM\SOFTWARE\GOG.com\Games",
        r"HKCU\SOFTWARE\GOG.com\Games",
    ];

    let mut games = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        let output = hidden_windows_command("reg")
            .args(["query", root, "/s"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                for game in parse_gog_registry_output(&text) {
                    if seen.insert(game.id.clone()) {
                        games.push(game);
                    }
                }
            }
            Ok(_) => {}
            Err(error) => warnings.push(format!("Could not query GOG registry data: {error}")),
        }
    }

    (games, warnings)
}

#[cfg(not(target_os = "windows"))]
fn scan_gog() -> (Vec<DiscoveredGame>, Vec<String>) {
    (Vec::new(), Vec::new())
}

#[cfg(target_os = "windows")]
fn resolve_windows_executable(name: &str, candidates: &[PathBuf]) -> Option<PathBuf> {
    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate.clone());
        }
    }

    let output = hidden_windows_command("where").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn scan_emulators() -> (Vec<DiscoveredGame>, Vec<String>) {
    let mut games = Vec::new();

    let program_files = env::var("PROGRAMFILES").ok().map(PathBuf::from);
    let program_files_x86 = env::var("PROGRAMFILES(X86)").ok().map(PathBuf::from);
    let local_app_data = env::var("LOCALAPPDATA").ok().map(PathBuf::from);
    let app_data = env::var("APPDATA").ok().map(PathBuf::from);

    let specs: Vec<(&str, &str, Vec<PathBuf>)> = vec![
        (
            "Dolphin Emulator",
            "Dolphin.exe",
            [
                program_files.as_ref().map(|p| p.join("Dolphin").join("Dolphin.exe")),
                local_app_data.as_ref().map(|p| p.join("Dolphin").join("Dolphin.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        (
            "PCSX2",
            "pcsx2-qt.exe",
            [
                program_files.as_ref().map(|p| p.join("PCSX2").join("pcsx2-qt.exe")),
                local_app_data.as_ref().map(|p| p.join("PCSX2").join("pcsx2-qt.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        (
            "RetroArch",
            "retroarch.exe",
            [
                program_files.as_ref().map(|p| p.join("RetroArch-Win64").join("retroarch.exe")),
                program_files_x86.as_ref().map(|p| p.join("RetroArch-Win64").join("retroarch.exe")),
                app_data.as_ref().map(|p| p.join("RetroArch").join("retroarch.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        (
            "Ryujinx",
            "Ryujinx.exe",
            [
                local_app_data.as_ref().map(|p| p.join("Ryujinx").join("Ryujinx.exe")),
                app_data.as_ref().map(|p| p.join("Ryujinx").join("Ryujinx.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        (
            "Cemu",
            "Cemu.exe",
            [
                program_files.as_ref().map(|p| p.join("Cemu").join("Cemu.exe")),
                local_app_data.as_ref().map(|p| p.join("Cemu").join("Cemu.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        (
            "PPSSPP",
            "PPSSPPWindows64.exe",
            [
                program_files.as_ref().map(|p| p.join("PPSSPP").join("PPSSPPWindows64.exe")),
                program_files_x86.as_ref().map(|p| p.join("PPSSPP").join("PPSSPPWindows64.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        (
            "DuckStation",
            "duckstation-qt-x64-ReleaseLTCG.exe",
            [
                local_app_data.as_ref().map(|p| p.join("DuckStation").join("duckstation-qt-x64-ReleaseLTCG.exe")),
                program_files.as_ref().map(|p| p.join("DuckStation").join("duckstation-qt-x64-ReleaseLTCG.exe")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
    ];

    for (title, executable_name, candidates) in specs {
        if let Some(executable) = resolve_windows_executable(executable_name, &candidates) {
            let install_path = executable
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_string_lossy()
                .into_owned();

            let source_id = title
                .to_ascii_lowercase()
                .replace(' ', "-");

            games.push(DiscoveredGame {
                id: format!("emulator:{source_id}"),
                title: title.to_string(),
                exe_path: Some(executable.to_string_lossy().into_owned()),
                install_path,
                source: "emulator".into(),
                source_id: Some(source_id),
            });
        }
    }

    (games, Vec::new())
}

#[cfg(not(target_os = "windows"))]
fn scan_emulators() -> (Vec<DiscoveredGame>, Vec<String>) {
    (Vec::new(), Vec::new())
}


fn likely_game_folder_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ![
        "windows",
        "program files",
        "program files (x86)",
        "programdata",
        "users",
        "$recycle.bin",
        "system volume information",
    ]
    .iter()
    .any(|blocked| lower == *blocked)
}

fn common_device_game_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(target_os = "windows")]
    {
        for letter in b'C'..=b'Z' {
            let drive = PathBuf::from(format!("{}:\\", letter as char));
            if !drive.exists() {
                continue;
            }

            for relative in [
                "Games", "Game", "PortableGames", "Portable Games",
                "XboxGames", "EA Games", "Ubisoft Games",
                "Program Files\\EA Games",
                "Program Files (x86)\\Ubisoft\\Ubisoft Game Launcher\\games",
            ] {
                let candidate = drive.join(relative);
                if candidate.is_dir() {
                    roots.push(candidate);
                }
            }
        }

        if let Ok(profile) = env::var("USERPROFILE") {
            let profile = PathBuf::from(profile);
            for relative in [
                "Games",
                "Desktop\\Games",
                "Documents\\Games",
                "Downloads",
                "Downloads\\Games",
            ] {
                let candidate = profile.join(relative);
                if candidate.is_dir() {
                    roots.push(candidate);
                }
            }
        }
    }

    roots.sort();
    roots.dedup();
    roots
}

fn scan_common_device_game_folders() -> (Vec<DiscoveredGame>, Vec<String>) {
    let started = Instant::now();
    let budget = Duration::from_secs(12);
    let mut games = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_paths = HashSet::new();
    let mut inspected_folders = 0_usize;

    for root in common_device_game_roots() {
        if started.elapsed() >= budget || inspected_folders >= 250 {
            warnings.push(
                "Automatic device-folder scan stopped at its safety limit; launcher scans still completed."
                    .into(),
            );
            break;
        }

        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.filter_map(Result::ok) {
            if started.elapsed() >= budget || inspected_folders >= 250 {
                break;
            }

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let title = entry.file_name().to_string_lossy().trim().to_string();
            if title.is_empty() || !likely_game_folder_name(&title) {
                continue;
            }

            inspected_folders += 1;

            let canonical = path.canonicalize().unwrap_or(path.clone());
            let canonical_key = canonical.to_string_lossy().to_ascii_lowercase();
            if !seen_paths.insert(canonical_key.clone()) {
                continue;
            }

            let Some(executable) =
                find_best_executable_bounded(&canonical, &title, started, budget)
            else {
                continue;
            };

            let mut hasher = DefaultHasher::new();
            canonical_key.hash(&mut hasher);
            let source_id = format!("{:016x}", hasher.finish());
            games.push(DiscoveredGame {
                id: format!("device:{source_id}"),
                title,
                exe_path: Some(executable.to_string_lossy().into_owned()),
                install_path: canonical.to_string_lossy().into_owned(),
                source: "device".into(),
                source_id: Some(source_id),
            });
        }
    }

    (games, warnings)
}

#[tauri::command]
fn list_games(app: AppHandle) -> Result<Vec<GameRecord>, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT g.id, g.title, g.exe_path, g.install_path, g.source, g.source_id, g.favorite,
                   g.cover_path, g.added_at, g.last_played, g.total_seconds, g.launch_count
            FROM games g
            INNER JOIN profile_games pg ON pg.game_id = g.id
            WHERE pg.profile_id = ?1 AND g.hidden = 0
            ORDER BY LOWER(g.title)
            "#,
        )
        .map_err(|error| format!("Could not prepare game list: {error}"))?;

    let rows = statement
        .query_map(params![profile_id], row_to_game)
        .map_err(|error| format!("Could not load games: {error}"))?;

    let mut games = Vec::new();
    for row in rows {
        games.push(row.map_err(|error| format!("Could not decode game: {error}"))?);
    }
    Ok(games)
}


fn normalized_discovery_path(value: &str) -> String {
    value.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase()
}

fn source_priority(source: &str) -> i32 {
    match source {
        "steam" => 100,
        "epic" => 95,
        "gog" => 90,
        "emulator" => 70,
        "device" => 20,
        _ => 10,
    }
}

fn dedupe_discovered_games(games: Vec<DiscoveredGame>) -> Vec<DiscoveredGame> {
    let mut by_key: HashMap<String, DiscoveredGame> = HashMap::new();
    for game in games {
        let exe_key = game.exe_path.as_deref().map(normalized_discovery_path).unwrap_or_default();
        let install_key = normalized_discovery_path(&game.install_path);
        let key = if !exe_key.is_empty() { format!("exe:{exe_key}") } else { format!("dir:{install_key}") };

        match by_key.get(&key) {
            Some(existing) if source_priority(&existing.source) >= source_priority(&game.source) => {}
            _ => { by_key.insert(key, game); }
        }
    }
    by_key.into_values().collect()
}

fn hide_stale_auto_duplicates(connection: &Connection, discovered: &[DiscoveredGame]) {
    let launcher_paths: HashSet<String> = discovered.iter()
        .filter(|g| matches!(g.source.as_str(), "steam" | "epic" | "gog"))
        .flat_map(|g| {
            let mut keys = vec![normalized_discovery_path(&g.install_path)];
            if let Some(exe) = g.exe_path.as_deref() { keys.push(normalized_discovery_path(exe)); }
            keys
        })
        .collect();

    if launcher_paths.is_empty() { return; }

    let mut statement = match connection.prepare(
        "SELECT id, exe_path, install_path FROM games WHERE hidden = 0 AND source = 'device'"
    ) { Ok(value) => value, Err(_) => return };

    let rows = match statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?))
    }) { Ok(value) => value, Err(_) => return };

    for row in rows.flatten() {
        let (id, exe, install) = row;
        let duplicate = launcher_paths.contains(&normalized_discovery_path(&install))
            || exe.as_deref().map(normalized_discovery_path).map(|p| launcher_paths.contains(&p)).unwrap_or(false);
        if duplicate {
            let _ = connection.execute("UPDATE games SET hidden = 1 WHERE id = ?1", params![id]);
        }
    }
}

fn scan_games_blocking(app: AppHandle) -> Result<ScanResult, String> {
    let (steam_games, mut warnings) = scan_steam();
    let (epic_games, epic_warnings) = scan_epic();
    let (gog_games, gog_warnings) = scan_gog();
    let (emulator_games, emulator_warnings) = scan_emulators();
    let (device_games, device_warnings) = scan_common_device_game_folders();

    warnings.extend(epic_warnings);
    warnings.extend(gog_warnings);
    warnings.extend(emulator_warnings);
    warnings.extend(device_warnings);

    let steam_found = steam_games.len();
    let epic_found = epic_games.len();
    let gog_found = gog_games.len();
    let emulator_found = emulator_games.len();
    let device_found = device_games.len();

    let all_games: Vec<DiscoveredGame> = dedupe_discovered_games(
        steam_games
            .into_iter()
            .chain(epic_games)
            .chain(gog_games)
            .chain(emulator_games)
            .chain(device_games)
            .collect()
    );

    let connection = open_database(&app)?;
    hide_stale_auto_duplicates(&connection, &all_games);
    let mut added = 0_usize;
    let mut updated = 0_usize;

    for game in &all_games {
        if upsert_discovered(&connection, game)? {
            updated += 1;
        } else {
            added += 1;
        }
        auto_apply_game_artwork(&app, &connection, game);
    }

    Ok(ScanResult {
        found: all_games.len(),
        added,
        updated,
        steam_found,
        epic_found,
        gog_found,
        emulator_found,
        device_found,
        warnings,
    })
}

#[tauri::command]
async fn scan_games(app: AppHandle) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan_games_blocking(app))
        .await
        .map_err(|error| format!("Game scan worker failed: {error}"))?
}

fn refresh_missing_covers_blocking(app: AppHandle) -> Result<ArtworkRefreshResult, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, exe_path, install_path, source, source_id, cover_path, cover_origin
             FROM games WHERE hidden = 0",
        )
        .map_err(|error| format!("Could not prepare missing-cover refresh: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                DiscoveredGame {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    exe_path: row.get(2)?,
                    install_path: row.get(3)?,
                    source: row.get(4)?,
                    source_id: row.get(5)?,
                },
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|error| format!("Could not load games for missing-cover refresh: {error}"))?;

    let games = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode missing-cover refresh rows: {error}"))?;
    drop(statement);

    let mut attempted = 0_usize;
    let mut updated = 0_usize;

    for (game, cover_path, cover_origin) in games {
        if cover_origin.as_deref() == Some("manual")
            && cover_path.as_deref().map(|path| Path::new(path).exists()).unwrap_or(false)
        {
            continue;
        }

        let missing = cover_path
            .as_deref()
            .map(|path| !Path::new(path).exists())
            .unwrap_or(true);
        if !missing {
            continue;
        }

        attempted += 1;
        auto_apply_game_artwork(&app, &connection, &game);

        let refreshed: Option<String> = connection
            .query_row(
                "SELECT cover_path FROM games WHERE id = ?1",
                params![game.id],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten();

        if refreshed.as_deref().map(|path| Path::new(path).exists()).unwrap_or(false) {
            updated += 1;
        }
    }

    Ok(ArtworkRefreshResult {
        attempted,
        updated,
        remaining: attempted.saturating_sub(updated),
    })
}

#[tauri::command]
async fn refresh_missing_covers(app: AppHandle) -> Result<ArtworkRefreshResult, String> {
    tauri::async_runtime::spawn_blocking(move || refresh_missing_covers_blocking(app))
        .await
        .map_err(|error| format!("Cover AI worker failed: {error}"))?
}



#[tauri::command]
fn list_profiles(app: AppHandle) -> Result<Vec<ProfileRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT p.id, p.name, p.created_at, p.last_used_at,
                   COUNT(DISTINCT pg.game_id) AS game_count,
                   COUNT(DISTINCT psb.id) AS backup_count
            FROM profiles p
            LEFT JOIN profile_games pg ON pg.profile_id = p.id
            LEFT JOIN profile_save_backups psb ON psb.profile_id = p.id
            GROUP BY p.id, p.name, p.created_at, p.last_used_at
            ORDER BY p.last_used_at DESC, LOWER(p.name)
            "#,
        )
        .map_err(|error| format!("Could not prepare profile list: {error}"))?;
    let rows = statement
        .query_map([], row_to_profile)
        .map_err(|error| format!("Could not load profiles: {error}"))?;
    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|error| format!("Could not decode profile: {error}"))?);
    }
    Ok(profiles)
}

#[tauri::command]
fn get_active_profile(app: AppHandle) -> Result<ProfileRecord, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    connection
        .query_row(
            r#"
            SELECT p.id, p.name, p.created_at, p.last_used_at,
                   COUNT(DISTINCT pg.game_id),
                   COUNT(DISTINCT psb.id)
            FROM profiles p
            LEFT JOIN profile_games pg ON pg.profile_id = p.id
            LEFT JOIN profile_save_backups psb ON psb.profile_id = p.id
            WHERE p.id = ?1
            GROUP BY p.id, p.name, p.created_at, p.last_used_at
            "#,
            params![profile_id],
            row_to_profile,
        )
        .map_err(|error| format!("Could not load active profile: {error}"))
}

#[tauri::command]
fn create_profile(app: AppHandle, name: String) -> Result<ProfileRecord, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Profile name cannot be empty.".into());
    }

    let connection = open_database(&app)?;
    let duplicate: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE LOWER(name) = LOWER(?1))",
            params![name],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check profile name: {error}"))?;
    if duplicate {
        return Err("A profile with that name already exists.".into());
    }

    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    connection
        .execute(
            "INSERT INTO profiles (id, name, created_at, last_used_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, timestamp, timestamp],
        )
        .map_err(|error| format!("Could not create profile: {error}"))?;

    connection
        .execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_profile_id', ?1)",
            params![id],
        )
        .map_err(|error| format!("Could not activate profile: {error}"))?;

    get_active_profile(app)
}

#[tauri::command]
fn rename_profile(app: AppHandle, profile_id: String, name: String) -> Result<ProfileRecord, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Profile name cannot be empty.".into());
    }
    if name.chars().count() > 48 {
        return Err("Profile name must be 48 characters or fewer.".into());
    }

    let connection = open_database(&app)?;
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = ?1)",
            params![profile_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check profile: {error}"))?;
    if !exists {
        return Err("Profile not found.".into());
    }

    let duplicate: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id != ?1 AND LOWER(name) = LOWER(?2))",
            params![profile_id, name],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check profile name: {error}"))?;
    if duplicate {
        return Err("A profile with that name already exists.".into());
    }

    connection
        .execute(
            "UPDATE profiles SET name = ?1, last_used_at = ?2 WHERE id = ?3",
            params![name, now(), profile_id],
        )
        .map_err(|error| format!("Could not rename profile: {error}"))?;

    drop(connection);
    list_profiles(app)?
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "Renamed profile could not be reloaded.".to_string())
}

#[tauri::command]
fn set_active_profile(app: AppHandle, profile_id: String) -> Result<ProfileRecord, String> {
    let connection = open_database(&app)?;
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = ?1)",
            params![profile_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check profile: {error}"))?;
    if !exists {
        return Err("Profile not found.".into());
    }

    connection
        .execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_profile_id', ?1)",
            params![profile_id],
        )
        .map_err(|error| format!("Could not switch profile: {error}"))?;
    connection
        .execute(
            "UPDATE profiles SET last_used_at = ?1 WHERE id = ?2",
            params![now(), profile_id],
        )
        .map_err(|error| format!("Could not update profile: {error}"))?;
    drop(connection);
    get_active_profile(app)
}

#[tauri::command]
fn delete_profile(app: AppHandle, profile_id: String) -> Result<ProfileRecord, String> {
    let connection = open_database(&app)?;
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
        .map_err(|error| format!("Could not count profiles: {error}"))?;
    if count <= 1 {
        return Err("Dusk must keep at least one profile.".into());
    }

    let active = active_profile_id(&connection)?;
    let fallback: String = connection
        .query_row(
            "SELECT id FROM profiles WHERE id != ?1 ORDER BY last_used_at DESC LIMIT 1",
            params![profile_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not choose fallback profile: {error}"))?;

    if active == profile_id {
        connection
            .execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_profile_id', ?1)",
                params![fallback],
            )
            .map_err(|error| format!("Could not switch fallback profile: {error}"))?;
    }

    connection
        .execute("DELETE FROM profiles WHERE id = ?1", params![profile_id])
        .map_err(|error| format!("Could not delete profile: {error}"))?;
    drop(connection);

    let data_dir = app_data_dir(&app)?;
    let backup_dir = data_dir.join("save-backups").join(&profile_id);
    if backup_dir.is_dir() {
        let _ = fs::remove_dir_all(backup_dir);
    }
    let profile_dir = data_dir.join("profiles").join(&profile_id);
    if profile_dir.is_dir() {
        let _ = fs::remove_dir_all(profile_dir);
    }

    get_active_profile(app)
}


#[tauri::command]
fn choose_game_installer() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("Game installer", &["exe", "msi"])
        .pick_file()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn run_game_installer(installer_path: String) -> Result<(), String> {
    let path = PathBuf::from(installer_path.trim());
    if !path.is_file() {
        return Err("The selected installer does not exist.".into());
    }

    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Could not resolve installer path: {error}"))?;
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "exe" => {
            Command::new(&canonical)
                .current_dir(canonical.parent().unwrap_or_else(|| Path::new("")))
                .spawn()
                .map_err(|error| format!("Could not start installer: {error}"))?;
        }
        "msi" => {
            Command::new("msiexec")
                .arg("/i")
                .arg(&canonical)
                .spawn()
                .map_err(|error| format!("Could not start MSI installer: {error}"))?;
        }
        _ => return Err("Dusk only runs local .exe or .msi installers.".into()),
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn run_game_installer(_installer_path: String) -> Result<(), String> {
    Err("Local installer launching is currently implemented for Windows.".into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OnlineFixSearchResult {
    title: String,
    url: String,
    description: String,
}

fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn parse_online_fix_search_results(html: &str) -> Result<Vec<OnlineFixSearchResult>, String> {
    // DLE renders search hits inside <div class="news news-search">, distinct
    // from the unrelated sidebar recommendations and popular-game carousel.
    let listing = html
        .split("class=\"news news-search\"")
        .nth(1)
        .unwrap_or("");
    let hits = Regex::new(
        r#"(?s)<a class="big-link" href="(https://online-fix\.me/games/[^"]+\.html)"></a>.*?<h2 class="title">\s*(.*?)\s*</h2>"#
    ).map_err(|error| error.to_string())?;
    let tags = Regex::new(r"<[^>]+>").map_err(|error| error.to_string())?;
    let mut results = Vec::new();
    for hit in hits.captures_iter(listing) {
        let url = xml_unescape(hit.get(1).unwrap().as_str());
        let title = xml_unescape(&tags.replace_all(hit.get(2).unwrap().as_str(), ""))
            .trim().to_string();
        if title.is_empty() || results.iter().any(|entry: &OnlineFixSearchResult| entry.url == url) {
            continue;
        }
        results.push(OnlineFixSearchResult { title, url, description: String::new() });
        if results.len() >= 21 { break; }
    }
    Ok(results)
}

#[tauri::command]
async fn search_online_fix_games(query: String) -> Result<Vec<OnlineFixSearchResult>, String> {
    let search_term = query.trim().to_string();
    if search_term.chars().count() < 3 || search_term.chars().count() > 120 {
        return Err("Enter a game title (3–120 characters).".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(18))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Dusk/1.8")
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .get("https://online-fix.me/index.php")
            .query(&[
                ("do", "search"),
                ("subaction", "search"),
                ("story", search_term.as_str()),
            ])
            .send()
            .map_err(|error| format!("Online-Fix search is unreachable: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Online-Fix search could not be loaded: {error}"))?;
        let html = response.text().map_err(|error| error.to_string())?;
        if !html.contains("news-search") && !html.contains("fullsearch") {
            return Err("Online-Fix did not return a search page. Try opening the site in Dusk instead.".into());
        }
        parse_online_fix_search_results(&html)
    }).await.map_err(|error| error.to_string())?
}

#[cfg(test)]
mod online_fix_search_tests {
    use super::*;
    #[test]
    fn extracts_real_result_cards_without_sidebar_entries() {
        let html = r#"<div class="news news-search"><div class="article clr">
            <a class="big-link" href="https://online-fix.me/games/adventures/18205-how-to-fish-po-seti.html"></a>
            <div class="article-content"><a href="https://online-fix.me/games/adventures/18205-how-to-fish-po-seti.html"><h2 class="title">
            How to Fish по сети
            </h2></a></div></div></div>"#;
        let hits = parse_online_fix_search_results(html).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].title.starts_with("How to Fish"));
    }
}


#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OnlineFixDownloadLink {
    url: String,
    label: String,
    kind: String,
    recommended: bool,
}

fn parse_online_fix_download_links(html: &str) -> Result<Vec<OnlineFixDownloadLink>, String> {
    // The game's article contains the real download mirrors. Navigation,
    // comments and sidebar ads are outside this section and are not inspected.
    let article = html
        .split("class=\"full-story-content\"")
        .nth(1)
        .ok_or_else(|| "This listing does not contain a game download section.".to_string())?;
    let article = article.split("<!--QuoteEEnd-->").next().unwrap_or(article);
    let anchor = Regex::new(r#"(?s)<a\b[^>]*\bhref="([^"]+)"[^>]*>(.*?)</a>"#)
        .map_err(|error| error.to_string())?;
    let tags = Regex::new(r"<[^>]*>").map_err(|error| error.to_string())?;
    let mut found: Vec<OnlineFixDownloadLink> = Vec::new();
    for hit in anchor.captures_iter(article) {
        let url = xml_unescape(hit.get(1).unwrap().as_str());
        let label = xml_unescape(
            &tags.replace_all(hit.get(2).unwrap().as_str(), "")
        ).trim().to_string();
        let Ok(parsed) = reqwest::Url::parse(&url) else { continue; };
        if parsed.scheme() != "https" || !matches!(parsed.port(), None | Some(2053)) {
            continue;
        }
        let kind = match parsed.host_str() {
            Some("hosters.online-fix.me") if label.contains("Hosters") => "mirror",
            Some("drive.online-fix.me") if label.contains("Drive") => "game",
            Some("uploads.online-fix.me") if parsed.path().starts_with("/torrents/") && label.to_lowercase().contains("torrent") => "torrent",
            Some("uploads.online-fix.me") if parsed.path().starts_with("/uploads/") && label.to_lowercase().contains("фикс") => "fix",
            _ => continue,
        };
        if !found.iter().any(|entry| entry.url == url) {
            found.push(OnlineFixDownloadLink {
                url,
                label: match kind {
                    "game" => "Full game · Online-Fix Drive",
                    "mirror" => "Hosters · archives (may be fixes only)",
                    "torrent" => "Torrent · manual download",
                    _ => "Fix-only files · not the full game",
                }.into(),
                kind: kind.into(),
                recommended: kind == "game",
            });
        }
    }
    found.sort_by_key(|entry| match entry.kind.as_str() {
        "game" => 0, "mirror" => 1, "torrent" => 2, _ => 3
    });
    Ok(found)
}

#[tauri::command]
async fn get_online_fix_download_links(listing_url: String) -> Result<Vec<OnlineFixDownloadLink>, String> {
    let parsed = verified_online_fix_listing_url(&listing_url)?;
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Dusk/1.8")
            .build()
            .map_err(|error| error.to_string())?;
        let response = client.get(parsed)
            .send()
            .map_err(|error| format!("Could not load game download sources: {error}"))?
            .error_for_status()
            .map_err(|error| format!("The game listing was not available: {error}"))?;
        let body = response.text().map_err(|error| error.to_string())?;
        if body.len() > 4_000_000 {
            return Err("The game listing is too large to inspect safely.".into());
        }
        parse_online_fix_download_links(&body)
    }).await.map_err(|error| error.to_string())?
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OnlineFixHosterFile {
    provider: String,
    filename: String,
    url: String,
    is_fix: bool,
    direct_archive: bool,
    requires_caution: bool,
}

fn parse_online_fix_hoster_files(html: &str) -> Result<Vec<OnlineFixHosterFile>, String> {
    // Hosters publishes the human-visible file list in the selected-provider
    // controls. This is not the HTML search results or any advertisement.
    let option_re = Regex::new(r#"(?s)<div\s+class="option[^"]*"[^>]*data-links="([^"]+)"[^>]*>([^<]+)</div>"#)
        .map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for option in option_re.captures_iter(html) {
        let provider = option.get(2).unwrap().as_str().trim();
        let raw = xml_unescape(option.get(1).unwrap().as_str());
        let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else { continue };
        for item in entries {
            let Some(url) = item.get("direct_link").and_then(|value| value.as_str()) else { continue };
            let Some(filename) = item.get("file_name").and_then(|value| value.as_str()) else { continue };
            let Ok(parsed) = reqwest::Url::parse(url) else { continue };
            let expected = match provider {
                "FileDitch" => "fileditchfiles.st",
                "FileKeeper" => "filekeeper.net",
                "Pixeldrain" => "pixeldrain.com",
                "Gofile" => "gofile.io",
                "VikingFile" => "vikingfile.com",
                _ => continue,
            };
            if parsed.scheme() != "https" || parsed.host_str() != Some(expected)
                || parsed.username() != "" || parsed.password().is_some()
                || filename.len() > 230
                || filename.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '|' | '?' | '*'))
                || !download_manager::archive_name(filename)
            {
                continue;
            }
            let lower = filename.to_ascii_lowercase();
            let is_fix = lower.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| matches!(word, "fix" | "repair" | "update" | "updates" | "patch" | "crack" | "redist"));
            let direct_archive = parsed.path_segments()
                .and_then(|mut segments| segments.next_back())
                .is_some_and(download_manager::archive_name);
            if files.iter().any(|entry: &OnlineFixHosterFile| entry.url == url) { continue; }
            files.push(OnlineFixHosterFile {
                provider: provider.into(), filename: filename.into(), url: url.into(),
                is_fix, direct_archive,
                requires_caution: item.get("is_dangerous").and_then(|value| value.as_bool()).unwrap_or(false),
            });
        }
    }
    files.sort_by_key(|entry| (
        entry.is_fix,
        entry.requires_caution,
        !entry.direct_archive,
        entry.provider != "FileDitch",
    ));
    files.truncate(80);
    Ok(files)
}

#[tauri::command]
async fn get_online_fix_hoster_files(hosters_url: String) -> Result<Vec<OnlineFixHosterFile>, String> {
    let url = reqwest::Url::parse(&hosters_url).map_err(|_| "Invalid Hosters URL.")?;
    if url.scheme() != "https" || url.host_str() != Some("hosters.online-fix.me")
        || !matches!(url.port(), None | Some(2053))
        || url.username() != "" || url.password().is_some() {
        return Err("Only official Online-Fix Hosters listings are supported.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Dusk/1.8")
            .build().map_err(|e| e.to_string())?;
        let response = client.get(url).send()
            .map_err(|e| format!("Cannot load Hosters file list: {e}"))?
            .error_for_status().map_err(|e| format!("Hosters refused the file list: {e}"))?;
        if response.content_length().unwrap_or(0) > 2_000_000 { return Err("Hosters page is too large.".into()); }
        let html = response.text().map_err(|e| e.to_string())?;
        if html.len() > 2_000_000 { return Err("Hosters page is too large.".into()); }
        let files = parse_online_fix_hoster_files(&html)?;
        if files.is_empty() { return Err("No recognizable archive files found on Hosters.".into()); }
        Ok(files)
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod hosters_file_tests {
    use super::*;

    #[test]
    fn finds_archive_not_fix_and_ignores_fake_host() {
        let html = r#"<div class="option selected" data-links="[{&quot;direct_link&quot;:&quot;https://fileditchfiles.st/f/My.Game.rar&quot;,&quot;file_name&quot;:&quot;My.Game.rar&quot;,&quot;is_dangerous&quot;:false},{&quot;direct_link&quot;:&quot;https://fileditchfiles.st/f/MyGame_Fix_Repair.rar&quot;,&quot;file_name&quot;:&quot;MyGame_Fix_Repair.rar&quot;,&quot;is_dangerous&quot;:false}]" data-id="1">FileDitch</div>
        <div class="option" data-links="[{&quot;direct_link&quot;:&quot;https://fileditchfiles.st.evil.org/ads/Bad.rar&quot;,&quot;file_name&quot;:&quot;Bad.rar&quot;}]" data-id="2">FileDitch</div>"#;
        let files = parse_online_fix_hoster_files(html).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename, "My.Game.rar");
        assert!(files[0].direct_archive);
        assert!(!files[0].is_fix);
        assert!(files[1].is_fix);
    }
}

fn verified_online_fix_listing_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid game listing URL.")?;
    if parsed.scheme() != "https"
        || !matches!(parsed.host_str(), Some("online-fix.me" | "www.online-fix.me"))
        || parsed.port().is_some()
        || !parsed.path().starts_with("/games/")
        || !parsed.path().ends_with(".html")
    {
        return Err("Only Online-Fix game listing pages are accepted.".into());
    }
    Ok(parsed)
}

#[cfg(test)]
mod online_fix_download_tests {
    use super::*;

    #[test]
    fn ranks_full_game_before_fix_and_torrent() {
        let sample = r#"<div class="full-story-content"><div itemprop="articleBody">
           <a href="https://uploads.online-fix.me:2053/uploads/How%20to%20Fish/">Скачать фикс с сервера</a>
           <a href="https://uploads.online-fix.me:2053/torrents/How%20to%20Fish/">Скачать Torrent</a>
           <a target="_blank" href="https://drive.online-fix.me:2053/How%20to%20Fish">Скачать с Online-Fix Drive</a>
           <a target="_blank" href="https://hosters.online-fix.me:2053/How%20to%20Fish">Скачать с Online-Fix Hosters</a>
           <!--QuoteEEnd--></div>"#;
        let sources = parse_online_fix_download_links(sample).unwrap();
        assert_eq!(sources.len(), 4);
        assert_eq!(sources[0].kind, "game");
        assert!(sources[0].recommended);
        assert!(sources[0].url.contains("drive.online-fix.me"));
        assert_eq!(sources[1].kind, "mirror");
        assert_eq!(sources[2].kind, "torrent");
        assert_eq!(sources[3].kind, "fix");
    }

    #[test]
    fn ignores_ad_hosts_unrelated_links_and_bad_urls() {
        let sample = r#"<a href="https://hosters.online-fix.me:2053/AD">Скачать с Online-Fix Hosters</a>
           <div class="full-story-content">
           <a href="https://advertising.example/download">Скачать с Online-Fix Hosters</a>
           <a href="http://hosters.online-fix.me:2053/Game">Скачать с Online-Fix Hosters</a>
           <a href="https://evilhosters.online-fix.me.evil.org/Game">Скачать с Online-Fix Hosters</a>
           <!--QuoteEEnd--></div>"#;
        assert!(parse_online_fix_download_links(sample).unwrap().is_empty());
        assert!(verified_online_fix_listing_url("https://online-fix.me/guides/faq").is_err());
        assert!(verified_online_fix_listing_url("https://online-fix.me.evil.com/games/g.html").is_err());
    }
}

fn verified_online_fix_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid listing URL.")?;
    if parsed.scheme() != "https"
        || !matches!(parsed.port(), None | Some(2053))
        || !matches!(parsed.host_str(), Some(
            "online-fix.me" | "www.online-fix.me"
            | "hosters.online-fix.me" | "drive.online-fix.me"
            | "uploads.online-fix.me" | "fileditchfiles.st"
            | "filekeeper.net" | "pixeldrain.com"
            | "gofile.io" | "vikingfile.com"
        ))
    {
        return Err("Only secure online-fix.me listing URLs are allowed.".into());
    }
    Ok(parsed)
}

// An isolated remote webview window keeps browsing inside Dusk without loading
// untrusted remote content into the privileged local game-library window.
// Official pages can navigate in the embedded browser. File downloads may
// originate from a different HTTPS CDN due to ordinary HTTP redirects.
fn is_online_fix_site(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && matches!(url.host_str(), Some(
        "online-fix.me" | "www.online-fix.me"
        | "hosters.online-fix.me" | "drive.online-fix.me"
        | "uploads.online-fix.me"
    ))
}

fn is_verified_file_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.username() == "" && url.password().is_none()
        && matches!(url.host_str(), Some(
            "fileditchfiles.st" | "filekeeper.net" | "pixeldrain.com"
            | "gofile.io" | "vikingfile.com"
        ))
}

fn is_https_archive_url(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" || known_ad_network(url.host_str().unwrap_or_default()) {
        return false;
    }
    let path = url.path().to_ascii_lowercase();
    [".zip", ".rar", ".7z", ".001"].iter().any(|suffix| path.ends_with(suffix))
}

fn known_ad_network(host: &str) -> bool {
    const BLOCKED: &[&str] = &[
        "exoclick.com", "exosrv.com", "magsrv.com", "realsrv.com",
        "juicyads.com", "adsterra.com", "propellerads.com",
        "onclickads.net", "popads.net", "popcash.net",
        "trafficjunky.net", "clickadu.com",
    ];
    BLOCKED.iter().any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
}

fn is_safe_game_archive_download(url: &reqwest::Url, filename: &str) -> bool {
    let allowed_origin = match url.scheme() {
        "https" => !known_ad_network(url.host_str().unwrap_or_default()),
        // Some WebView2 file hosts create local ZIP blobs after a server-side
        // handshake. Only accept blobs originating on the official hosts.
        "blob" => reqwest::Url::parse(url.path())
            .map(|inner| is_online_fix_site(&inner))
            .unwrap_or(false),
        _ => false,
    };
    if !allowed_origin { return false; }
    if filename.is_empty()
        || filename.len() > 240
        || filename == "." || filename == ".."
        || filename.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
    {
        return false;
    }
    let lower = filename.to_ascii_lowercase();
    let allowed_archive = [".zip", ".rar", ".7z", ".7z.001"]
        .iter().any(|ext| lower.ends_with(ext));
    allowed_archive
}

#[cfg(test)]
mod online_fix_download_validation_tests {
    use super::*;

    #[test]
    fn accepts_signed_cdn_downloads_with_safe_archive_names() {
        let url = reqwest::Url::parse("https://cdn.example.net/api/dl?signature=xyz").unwrap();
        assert!(is_safe_game_archive_download(&url, "How to Fish.part1.rar"));
        assert!(is_safe_game_archive_download(&url, "How to Fish.zip"));
        assert!(is_safe_game_archive_download(&url, "How to Fish.7z.001"));
    }

    #[test]
    fn rejects_executables_ads_and_insecure_downloads() {
        let cdn = reqwest::Url::parse("https://cdn.example.net/file").unwrap();
        assert!(!is_safe_game_archive_download(&cdn, "setup.exe"));
        assert!(is_safe_game_archive_download(&cdn, "Game_fix_repair.zip"));
        // Fix-only files can be downloaded manually; they must not be imported
        // as full games by the frontend watcher.
        assert!(!is_safe_game_archive_download(&cdn, "../Game.zip"));
        let ad = reqwest::Url::parse("https://sub.exoclick.com/ads/Game.zip").unwrap();
        assert!(!is_safe_game_archive_download(&ad, "Game.zip"));
        let http = reqwest::Url::parse("http://cdn.example.net/Game.zip").unwrap();
        assert!(!is_safe_game_archive_download(&http, "Game.zip"));
    }
}

#[tauri::command]
async fn open_online_fix_result(
    app: AppHandle,
    url: String,
    game_title: Option<String>,
    auto_select: Option<bool>,
) -> Result<(), String> {
    let parsed = verified_online_fix_url(&url)?;
    let label = format!("online-fix-{}", Uuid::new_v4().simple());
    let download_title = game_title.unwrap_or_default();
    if auto_select.unwrap_or(false) && (download_title.trim().is_empty() || download_title.len() > 200) {
        return Err("A valid game title is required for automatic link selection.".into());
    }
    let mut builder = tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::External(parsed))
        .title("Dusk — Online-Fix downloads")
        .inner_size(1100.0, 760.0)
        .accept_first_mouse(true)
        .initialization_script(include_str!("online_fix_navigation.js"))
        .initialization_script(include_str!("online_fix_adblock.js"))
        .initialization_script(include_str!("browser_copy_link.js"))
        .on_navigation(|url| {
            // Only official site pages are browsable in Dusk. Signed HTTPS
            // archive URLs from a storage CDN may navigate directly to a file;
            // the download handler separately validates the filename.
            is_online_fix_site(url)
                || is_verified_file_host(url)
                || is_https_archive_url(url)
        })
        .on_new_window(|_url, _features| {
            // _blank download and navigation links are redirected to the
            // current window by our navigation script; ad popups are denied.
            tauri::webview::NewWindowResponse::Deny
        })
        .on_download(|_webview, event| match event {
            tauri::webview::DownloadEvent::Requested { url, destination } => {
                // WebView2 may download from a CDN with a signed HTTPS URL
                // rather than the original Drive/Hosters hostname.
                // It also derives the filename from Content-Disposition.
                let Some(file_name) = destination.file_name().and_then(|s| s.to_str()) else {
                    return false;
                };
                if !is_safe_game_archive_download(&url, file_name) {
                    return false;
                }
                let Some(home) = env::var_os("USERPROFILE") else { return false; };
                let downloads = PathBuf::from(home).join("Downloads");
                if !downloads.is_dir() { return false; }

                let mut target = downloads.join(file_name);
                if target.exists() {
                    // Do not silently reject a repeated download. WebView2
                    // can write a suffixed copy of a normal one-file archive;
                    // never rename multipart volumes, whose names must match.
                    let multipart = file_name.to_ascii_lowercase().contains(".part")
                        || file_name.to_ascii_lowercase().ends_with(".7z.001");
                    if multipart {
                        return false;
                    }
                    let extension = Path::new(file_name).extension().and_then(|s| s.to_str()).unwrap_or("zip");
                    let stem = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("game");
                    let mut unique = None;
                    for index in 2..100 {
                        let proposed = downloads.join(format!("{stem} ({index}).{extension}"));
                        if !proposed.exists() {
                            unique = Some(proposed);
                            break;
                        }
                    }
                    let Some(next) = unique else { return false };
                    target = next;
                }
                *destination = target;
                true
            }
            tauri::webview::DownloadEvent::Finished { url, path, success } => {
                if !success {
                    eprintln!("Online-Fix download failed: {url} -> {path:?}");
                }
                true
            }
            _ => true,
        });

    if auto_select.unwrap_or(false) {
        let title_literal = serde_json::to_string(&download_title)
            .map_err(|error| format!("Could not prepare game title: {error}"))?;
        builder = builder.initialization_script(
            include_str!("online_fix_autoselect.js").replace("__DUSK_TITLE__", &title_literal)
        );
    }

    builder.build()
        .map_err(|error| format!("Could not open the in-app browser: {error}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn open_online_fix_browser(url: String) -> Result<(), String> {
    verified_online_fix_url(&url)?;
    hidden_windows_command("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn()
        .map_err(|error| format!("Could not open listing in browser: {error}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn open_online_fix_browser(_url: String) -> Result<(), String> {
    Err("External browser integration is currently implemented for Windows.".into())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn open_external_target(target: String) -> Result<(), String> {
    let url = match target.as_str() {
        "creator" => "https://guns.lol/bxane",
        "steam" => "https://store.steampowered.com/",
        "epic" => "https://store.epicgames.com/",
        "gog" => "https://www.gog.com/",
        "itch" => "https://itch.io/",
        _ => return Err("That external destination is not allowed.".into()),
    };

    hidden_windows_command("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map_err(|error| format!("Could not open link: {error}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn open_external_target(_target: String) -> Result<(), String> {
    Err("Opening external destinations is currently implemented for Windows.".into())
}

#[tauri::command]
fn choose_executable() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("Windows executable", &["exe"])
        .pick_file()
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
fn add_manual_game(app: AppHandle, title: String, exe_path: String) -> Result<GameRecord, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Game title cannot be empty.".into());
    }

    let executable = PathBuf::from(exe_path.trim());
    if !executable.is_file() {
        return Err("The selected executable does not exist.".into());
    }

    let install_path = executable
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy()
        .into_owned();

    let game = DiscoveredGame {
        id: format!("manual:{}", Uuid::new_v4()),
        title: title.to_string(),
        exe_path: Some(executable.to_string_lossy().into_owned()),
        install_path,
        source: "manual".into(),
        source_id: None,
    };

    let connection = open_database(&app)?;
    upsert_discovered(&connection, &game)?;
    get_game(&connection, &game.id)
}

#[tauri::command]
fn set_favorite(app: AppHandle, game_id: String, favorite: bool) -> Result<(), String> {
    let connection = open_database(&app)?;
    connection
        .execute(
            "UPDATE games SET favorite = ?1 WHERE id = ?2",
            params![favorite as i64, game_id],
        )
        .map_err(|error| format!("Could not update favorite: {error}"))?;
    Ok(())
}

#[tauri::command]
fn rename_game(app: AppHandle, game_id: String, title: String) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Game title cannot be empty.".into());
    }
    let connection = open_database(&app)?;
    connection
        .execute(
            "UPDATE games SET title = ?1 WHERE id = ?2",
            params![title, game_id],
        )
        .map_err(|error| format!("Could not rename game: {error}"))?;
    Ok(())
}

#[tauri::command]
fn remove_game(app: AppHandle, game_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    connection
        .execute(
            "DELETE FROM profile_games WHERE profile_id = ?1 AND game_id = ?2",
            params![profile_id, game_id],
        )
        .map_err(|error| format!("Could not remove game from this profile: {error}"))?;
    Ok(())
}

#[tauri::command]
fn choose_cover(app: AppHandle, game_id: String) -> Result<bool, String> {
    let source = match rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_file()
    {
        Some(path) => path,
        None => return Ok(false),
    };

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();

    let covers_dir = app_data_dir(&app)?.join("covers");
    fs::create_dir_all(&covers_dir)
        .map_err(|error| format!("Could not create cover directory: {error}"))?;

    let destination = covers_dir.join(format!("{}.{}", game_id.replace(':', "_"), extension));
    fs::copy(&source, &destination)
        .map_err(|error| format!("Could not copy cover image: {error}"))?;

    let connection = open_database(&app)?;
    connection
        .execute(
            "UPDATE games SET cover_path = ?1, cover_origin = 'manual' WHERE id = ?2",
            params![destination.to_string_lossy().into_owned(), game_id],
        )
        .map_err(|error| format!("Could not save cover image: {error}"))?;

    Ok(true)
}

#[cfg(target_os = "windows")]
fn process_running(executable_name: &str) -> bool {
    let filter = format!("IMAGENAME eq {executable_name}");
    let output = hidden_windows_command("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output();

    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains(&executable_name.to_ascii_lowercase()),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn process_running(_executable_name: &str) -> bool {
    false
}

fn record_session(app: &AppHandle, game_id: &str, started_at: DateTime<Utc>, duration: Duration) {
    let seconds = duration.as_secs() as i64;
    if seconds <= 0 {
        return;
    }

    let ended_at = Utc::now();
    if let Ok(connection) = open_database(app) {
        let _ = connection.execute(
            r#"
            INSERT INTO sessions (game_id, started_at, ended_at, duration_seconds)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                game_id,
                started_at.to_rfc3339(),
                ended_at.to_rfc3339(),
                seconds
            ],
        );

        let _ = connection.execute(
            r#"
            UPDATE games
            SET total_seconds = total_seconds + ?1,
                last_played = ?2,
                launch_count = launch_count + 1
            WHERE id = ?3
            "#,
            params![seconds, ended_at.to_rfc3339(), game_id],
        );
    }
}

#[cfg(target_os = "windows")]
fn launch_steam_game(
    app: AppHandle,
    game: GameRecord,
) -> Result<LaunchResult, String> {
    let app_id = game
        .source_id
        .clone()
        .ok_or_else(|| "Steam App ID is missing.".to_string())?;
    let uri = format!("steam://rungameid/{app_id}");

    hidden_windows_command("cmd")
        .args(["/C", "start", "", &uri])
        .spawn()
        .map_err(|error| format!("Could not ask Steam to launch the game: {error}"))?;

    let executable_name = game
        .exe_path
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned);

    let Some(executable_name) = executable_name else {
        return Ok(LaunchResult {
            started: true,
            tracking: false,
            message: "Launched through Steam. Dusk could not identify the game executable, so this session will not be counted.".into(),
        });
    };

    let game_id = game.id.clone();
    thread::spawn(move || {
        let discovery_deadline = Instant::now() + Duration::from_secs(90);
        while Instant::now() < discovery_deadline {
            if process_running(&executable_name) {
                let started_at = Utc::now();
                let timer = Instant::now();
                let mut consecutive_misses = 0;

                loop {
                    thread::sleep(Duration::from_secs(5));
                    if process_running(&executable_name) {
                        consecutive_misses = 0;
                    } else {
                        consecutive_misses += 1;
                        if consecutive_misses >= 2 {
                            break;
                        }
                    }
                }

                record_session(&app, &game_id, started_at, timer.elapsed());
                return;
            }
            thread::sleep(Duration::from_secs(2));
        }
    });

    Ok(LaunchResult {
        started: true,
        tracking: true,
        message: "Steam launch requested. Dusk will start counting once the game process appears.".into(),
    })
}

#[cfg(not(target_os = "windows"))]
fn launch_steam_game(_app: AppHandle, _game: GameRecord) -> Result<LaunchResult, String> {
    Err("Steam launching is currently implemented for Windows.".into())
}

fn launch_direct_game(app: AppHandle, game: GameRecord) -> Result<LaunchResult, String> {
    let exe_path = game
        .exe_path
        .clone()
        .ok_or_else(|| "No executable is known for this game.".to_string())?;
    let executable = PathBuf::from(&exe_path);

    if !executable.exists() {
        return Err("The game executable no longer exists. Rescan or add the game again.".into());
    }

    let working_directory = executable
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&game.install_path));

    let mut child = Command::new(&executable)
        .current_dir(working_directory)
        .spawn()
        .map_err(|error| format!("Could not launch game: {error}"))?;

    let game_id = game.id.clone();
    thread::spawn(move || {
        let started_at = Utc::now();
        let timer = Instant::now();
        if child.wait().is_ok() {
            record_session(&app, &game_id, started_at, timer.elapsed());
        }
    });

    Ok(LaunchResult {
        started: true,
        tracking: true,
        message: "Game launched. Dusk is tracking this session.".into(),
    })
}

#[tauri::command]
fn launch_game(app: AppHandle, game_id: String) -> Result<LaunchResult, String> {
    let connection = open_database(&app)?;
    let game = get_game(&connection, &game_id)?;
    drop(connection);

    if game.source == "steam" {
        launch_steam_game(app, game)
    } else {
        launch_direct_game(app, game)
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn open_game_folder(app: AppHandle, game_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let game = get_game(&connection, &game_id)?;

    Command::new("explorer")
        .arg(&game.install_path)
        .spawn()
        .map_err(|error| format!("Could not open game folder: {error}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn open_game_folder(_app: AppHandle, _game_id: String) -> Result<(), String> {
    Err("Opening game folders is currently implemented for Windows.".into())
}


fn is_screenshot_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("gif") | Some("bmp")
    )
}

fn screenshot_created_at(path: &Path) -> String {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(|modified| DateTime::<Utc>::from(modified).to_rfc3339())
        .unwrap_or_else(now)
}

enum ScreenshotStoreOutcome {
    Imported,
    Duplicate,
    Ignored,
    Failed,
}

fn store_screenshot_source(
    app: &AppHandle,
    connection: &Connection,
    game_id: &str,
    source: &Path,
    allow_ignored: bool,
) -> ScreenshotStoreOutcome {
    if !source.is_file() || !is_screenshot_image(source) {
        return ScreenshotStoreOutcome::Failed;
    }

    let source_path = source.to_string_lossy().into_owned();

    if allow_ignored {
        let _ = connection.execute(
            "DELETE FROM ignored_screenshot_sources WHERE source_path = ?1",
            params![source_path],
        );
    } else {
        let ignored: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ignored_screenshot_sources WHERE source_path = ?1)",
                params![source_path],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if ignored {
            return ScreenshotStoreOutcome::Ignored;
        }
    }

    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM screenshots WHERE source_path = ?1)",
            params![source_path],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if exists {
        return ScreenshotStoreOutcome::Duplicate;
    }

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();

    let screenshots_dir = match app_data_dir(app) {
        Ok(path) => path.join("screenshots").join(game_id.replace(':', "_")),
        Err(_) => return ScreenshotStoreOutcome::Failed,
    };

    if fs::create_dir_all(&screenshots_dir).is_err() {
        return ScreenshotStoreOutcome::Failed;
    }

    let destination = screenshots_dir.join(format!("{}.{}", Uuid::new_v4(), extension));
    if fs::copy(source, &destination).is_err() {
        return ScreenshotStoreOutcome::Failed;
    }

    let inserted = connection.execute(
        "INSERT OR IGNORE INTO screenshots (game_id, path, source_path, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            game_id,
            destination.to_string_lossy().into_owned(),
            source_path,
            screenshot_created_at(source)
        ],
    );

    match inserted {
        Ok(1) => ScreenshotStoreOutcome::Imported,
        Ok(_) => {
            let _ = fs::remove_file(destination);
            ScreenshotStoreOutcome::Duplicate
        }
        Err(_) => {
            let _ = fs::remove_file(destination);
            ScreenshotStoreOutcome::Failed
        }
    }
}

fn image_files_in_directory(directory: &Path, max_depth: usize) -> Vec<PathBuf> {
    if !directory.is_dir() {
        return Vec::new();
    }

    WalkDir::new(directory)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_screenshot_image(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn normalized_match_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn scan_screenshots_blocking(app: AppHandle) -> Result<AutoScreenshotScanResult, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, title, install_path, source, source_id
            FROM games
            WHERE hidden = 0
            "#,
        )
        .map_err(|error| format!("Could not prepare screenshot game scan: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|error| format!("Could not load games for screenshot scan: {error}"))?;

    let games: Vec<(String, String, String, String, Option<String>)> = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode games for screenshot scan: {error}"))?;
    drop(statement);

    let mut found = 0;
    let mut imported = 0;
    let mut steam_imported = 0;
    let mut matched_imported = 0;
    let mut skipped_duplicates = 0;
    let mut seen_sources = HashSet::new();

    // Steam keeps screenshots under userdata/<account>/760/remote/<appid>/screenshots.
    for (game_id, _title, _install_path, source, source_id) in &games {
        if source != "steam" {
            continue;
        }
        let Some(app_id) = source_id.as_deref() else {
            continue;
        };

        for steam_root in steam_roots() {
            let userdata = steam_root.join("userdata");
            let accounts = match fs::read_dir(&userdata) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for account in accounts.filter_map(Result::ok) {
                let directory = account
                    .path()
                    .join("760")
                    .join("remote")
                    .join(app_id)
                    .join("screenshots");

                for source_path in image_files_in_directory(&directory, 1) {
                    let key = source_path.to_string_lossy().into_owned();
                    if !seen_sources.insert(key) {
                        continue;
                    }
                    found += 1;
                    match store_screenshot_source(&app, &connection, game_id, &source_path, false) {
                        ScreenshotStoreOutcome::Imported => {
                            imported += 1;
                            steam_imported += 1;
                        }
                        ScreenshotStoreOutcome::Duplicate | ScreenshotStoreOutcome::Ignored => {
                            skipped_duplicates += 1;
                        }
                        ScreenshotStoreOutcome::Failed => {}
                    }
                }
            }
        }
    }

    // Match screenshots inside common per-game screenshot folders.
    for (game_id, _title, install_path, _source, _source_id) in &games {
        let root = PathBuf::from(install_path);
        for name in ["screenshots", "Screenshots", "ScreenShots", "captures", "Captures"] {
            let directory = root.join(name);
            for source_path in image_files_in_directory(&directory, 2) {
                let key = source_path.to_string_lossy().into_owned();
                if !seen_sources.insert(key) {
                    continue;
                }
                found += 1;
                match store_screenshot_source(&app, &connection, game_id, &source_path, false) {
                    ScreenshotStoreOutcome::Imported => {
                        imported += 1;
                        matched_imported += 1;
                    }
                    ScreenshotStoreOutcome::Duplicate | ScreenshotStoreOutcome::Ignored => {
                        skipped_duplicates += 1;
                    }
                    ScreenshotStoreOutcome::Failed => {}
                }
            }
        }
    }

    // Windows and Xbox Game Bar commonly save to these folders. To avoid false
    // associations, only import when the filename contains a sufficiently specific
    // normalized game title.
    if let Ok(profile) = env::var("USERPROFILE") {
        let profile = PathBuf::from(profile);
        let shared_directories = [
            profile.join("Pictures").join("Screenshots"),
            profile.join("Videos").join("Captures"),
        ];

        for directory in shared_directories {
            for source_path in image_files_in_directory(&directory, 2) {
                let key = source_path.to_string_lossy().into_owned();
                if !seen_sources.insert(key) {
                    continue;
                }

                let filename = source_path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                let normalized_file = normalized_match_text(filename);

                let matched_game = games.iter().find(|(_id, title, _path, _source, _source_id)| {
                    let normalized_title = normalized_match_text(title);
                    normalized_title.len() >= 5 && normalized_file.contains(&normalized_title)
                });

                let Some((game_id, _title, _path, _source, _source_id)) = matched_game else {
                    continue;
                };

                found += 1;
                match store_screenshot_source(&app, &connection, game_id, &source_path, false) {
                    ScreenshotStoreOutcome::Imported => {
                        imported += 1;
                        matched_imported += 1;
                    }
                    ScreenshotStoreOutcome::Duplicate | ScreenshotStoreOutcome::Ignored => {
                        skipped_duplicates += 1;
                    }
                    ScreenshotStoreOutcome::Failed => {}
                }
            }
        }
    }

    Ok(AutoScreenshotScanResult {
        found,
        imported,
        steam_imported,
        matched_imported,
        skipped_duplicates,
    })
}

#[tauri::command]
async fn scan_screenshots(app: AppHandle) -> Result<AutoScreenshotScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan_screenshots_blocking(app))
        .await
        .map_err(|error| format!("Screenshot scan worker failed: {error}"))?
}

#[tauri::command]
fn import_screenshots(app: AppHandle, game_id: String) -> Result<usize, String> {
    let selected = match rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
        .pick_files()
    {
        Some(files) => files,
        None => return Ok(0),
    };

    let connection = open_database(&app)?;
    let mut imported = 0;

    for source in selected {
        if matches!(
            store_screenshot_source(&app, &connection, &game_id, &source, true),
            ScreenshotStoreOutcome::Imported
        ) {
            imported += 1;
        }
    }

    Ok(imported)
}

#[tauri::command]
fn list_screenshots(app: AppHandle, game_id: Option<String>) -> Result<Vec<ScreenshotRecord>, String> {
    let connection = open_database(&app)?;

    let sql = if game_id.is_some() {
        "SELECT id, game_id, path, created_at FROM screenshots WHERE game_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, game_id, path, created_at FROM screenshots ORDER BY created_at DESC"
    };

    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("Could not prepare screenshot query: {error}"))?;

    let decode = |row: &rusqlite::Row<'_>| -> rusqlite::Result<ScreenshotRecord> {
        let path: String = row.get(2)?;
        Ok(ScreenshotRecord {
            id: row.get(0)?,
            game_id: row.get(1)?,
            data_url: None,
            path,
            created_at: row.get(3)?,
        })
    };

    let mut screenshots = Vec::new();

    if let Some(game_id) = game_id {
        let rows = statement
            .query_map(params![game_id], decode)
            .map_err(|error| format!("Could not load screenshots: {error}"))?;
        for row in rows {
            screenshots.push(row.map_err(|error| format!("Could not decode screenshot: {error}"))?);
        }
    } else {
        let rows = statement
            .query_map([], decode)
            .map_err(|error| format!("Could not load screenshots: {error}"))?;
        for row in rows {
            screenshots.push(row.map_err(|error| format!("Could not decode screenshot: {error}"))?);
        }
    }

    Ok(screenshots)
}

#[tauri::command]
fn delete_screenshot(app: AppHandle, screenshot_id: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    let screenshot: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT path, source_path FROM screenshots WHERE id = ?1",
            params![screenshot_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not find screenshot: {error}"))?;

    if let Some((path, source_path)) = screenshot {
        let _ = fs::remove_file(path);
        if let Some(source_path) = source_path {
            let _ = connection.execute(
                "INSERT OR REPLACE INTO ignored_screenshot_sources (source_path, ignored_at) VALUES (?1, ?2)",
                params![source_path, now()],
            );
        }
    }

    connection
        .execute("DELETE FROM screenshots WHERE id = ?1", params![screenshot_id])
        .map_err(|error| format!("Could not delete screenshot: {error}"))?;

    Ok(())
}

#[tauri::command]
fn list_collections(app: AppHandle) -> Result<Vec<CollectionRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT c.id, c.name, COUNT(cg.game_id)
            FROM collections c
            LEFT JOIN collection_games cg ON cg.collection_id = c.id
            GROUP BY c.id, c.name
            ORDER BY c.name COLLATE NOCASE ASC
            "#,
        )
        .map_err(|error| format!("Could not prepare collections: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(CollectionRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                game_count: row.get(2)?,
            })
        })
        .map_err(|error| format!("Could not read collections: {error}"))?;

    let mut collections = Vec::new();
    for row in rows {
        collections.push(row.map_err(|error| format!("Could not decode collection: {error}"))?);
    }
    Ok(collections)
}

#[tauri::command]
fn create_collection(app: AppHandle, name: String) -> Result<CollectionRecord, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Collection name cannot be empty.".into());
    }

    let id = Uuid::new_v4().to_string();
    let connection = open_database(&app)?;
    connection
        .execute(
            "INSERT INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id, name, now()],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE") {
                "A collection with that name already exists.".to_string()
            } else {
                format!("Could not create collection: {error}")
            }
        })?;

    Ok(CollectionRecord {
        id,
        name: name.to_string(),
        game_count: 0,
    })
}

#[tauri::command]
fn delete_collection(app: AppHandle, collection_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    connection
        .execute("DELETE FROM collections WHERE id = ?1", params![collection_id])
        .map_err(|error| format!("Could not delete collection: {error}"))?;
    Ok(())
}

#[tauri::command]
fn set_collection_membership(
    app: AppHandle,
    collection_id: String,
    game_id: String,
    included: bool,
) -> Result<(), String> {
    let connection = open_database(&app)?;
    if included {
        connection
            .execute(
                "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
                params![collection_id, game_id],
            )
            .map_err(|error| format!("Could not add game to collection: {error}"))?;
    } else {
        connection
            .execute(
                "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
                params![collection_id, game_id],
            )
            .map_err(|error| format!("Could not remove game from collection: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn collection_memberships(app: AppHandle) -> Result<Vec<CollectionMembership>, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT cg.collection_id, cg.game_id
            FROM collection_games cg
            INNER JOIN profile_games pg ON pg.game_id = cg.game_id
            WHERE pg.profile_id = ?1
            "#,
        )
        .map_err(|error| format!("Could not prepare collection memberships: {error}"))?;

    let rows = statement
        .query_map(params![profile_id], |row| {
            Ok(CollectionMembership {
                collection_id: row.get(0)?,
                game_id: row.get(1)?,
            })
        })
        .map_err(|error| format!("Could not load collection memberships: {error}"))?;

    let mut memberships = Vec::new();
    for row in rows {
        memberships.push(row.map_err(|error| format!("Could not decode membership: {error}"))?);
    }
    Ok(memberships)
}

#[tauri::command]
fn get_stats(app: AppHandle) -> Result<Stats, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;

    let game_count = connection
        .query_row(
            "SELECT COUNT(*) FROM profile_games pg INNER JOIN games g ON g.id = pg.game_id WHERE pg.profile_id = ?1 AND g.hidden = 0",
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let favorite_count = connection
        .query_row(
            "SELECT COUNT(*) FROM profile_games pg INNER JOIN games g ON g.id = pg.game_id WHERE pg.profile_id = ?1 AND g.hidden = 0 AND g.favorite = 1",
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let played_game_count = connection
        .query_row(
            "SELECT COUNT(*) FROM profile_games pg INNER JOIN games g ON g.id = pg.game_id WHERE pg.profile_id = ?1 AND g.hidden = 0 AND g.launch_count > 0",
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_seconds = connection
        .query_row(
            "SELECT COALESCE(SUM(g.total_seconds), 0) FROM profile_games pg INNER JOIN games g ON g.id = pg.game_id WHERE pg.profile_id = ?1 AND g.hidden = 0",
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let launch_count = connection
        .query_row(
            "SELECT COALESCE(SUM(g.launch_count), 0) FROM profile_games pg INNER JOIN games g ON g.id = pg.game_id WHERE pg.profile_id = ?1 AND g.hidden = 0",
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let last_7_days_seconds = connection
        .query_row(
            r#"
            SELECT COALESCE(SUM(s.duration_seconds), 0)
            FROM sessions s
            INNER JOIN profile_games pg ON pg.game_id = s.game_id
            WHERE pg.profile_id = ?1
              AND s.started_at >= datetime('now', '-7 days')
            "#,
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let screenshot_count = connection
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM screenshots s
            INNER JOIN profile_games pg ON pg.game_id = s.game_id
            WHERE pg.profile_id = ?1
            "#,
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let top_game = connection
        .query_row(
            r#"
            SELECT g.title
            FROM profile_games pg
            INNER JOIN games g ON g.id = pg.game_id
            WHERE pg.profile_id = ?1 AND g.hidden = 0 AND g.total_seconds > 0
            ORDER BY g.total_seconds DESC
            LIMIT 1
            "#,
            params![profile_id],
            |row| row.get(0),
        )
        .optional()
        .unwrap_or(None);

    Ok(Stats {
        game_count,
        favorite_count,
        played_game_count,
        total_seconds,
        launch_count,
        last_7_days_seconds,
        screenshot_count,
        top_game,
    })
}

#[tauri::command]
fn list_achievements(app: AppHandle) -> Result<Vec<Achievement>, String> {
    let stats = get_stats(app.clone())?;
    let connection = open_database(&app)?;

    let profile_id = active_profile_id(&connection)?;
    let night_sessions: i64 = connection
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM sessions s
            INNER JOIN profile_games pg ON pg.game_id = s.game_id
            WHERE pg.profile_id = ?1
              AND CAST(strftime('%H', s.started_at) AS INTEGER) BETWEEN 0 AND 4
            "#,
            params![profile_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut achievements = vec![
        Achievement {
            id: "first-launch".into(),
            title: "First Light".into(),
            description: "Finish your first tracked game session.".into(),
            unlocked: stats.launch_count >= 1,
            current: stats.launch_count.min(1),
            target: 1,
        },
        Achievement {
            id: "collector".into(),
            title: "Collector".into(),
            description: "Build a library of 10 games.".into(),
            unlocked: stats.game_count >= 10,
            current: stats.game_count.min(10),
            target: 10,
        },
        Achievement {
            id: "ten-hours".into(),
            title: "Settled In".into(),
            description: "Track 10 hours of playtime in Dusk.".into(),
            unlocked: stats.total_seconds >= 36_000,
            current: (stats.total_seconds / 3600).min(10),
            target: 10,
        },
        Achievement {
            id: "hundred-hours".into(),
            title: "After Dark".into(),
            description: "Track 100 hours of playtime in Dusk.".into(),
            unlocked: stats.total_seconds >= 360_000,
            current: (stats.total_seconds / 3600).min(100),
            target: 100,
        },
        Achievement {
            id: "variety".into(),
            title: "No Main".into(),
            description: "Play 5 different games.".into(),
            unlocked: stats.played_game_count >= 5,
            current: stats.played_game_count.min(5),
            target: 5,
        },
        Achievement {
            id: "snapshots".into(),
            title: "Memory Card".into(),
            description: "Import 10 screenshots into Dusk.".into(),
            unlocked: stats.screenshot_count >= 10,
            current: stats.screenshot_count.min(10),
            target: 10,
        },
        Achievement {
            id: "night-shift".into(),
            title: "Night Shift".into(),
            description: "Finish a tracked session that started between midnight and 05:00.".into(),
            unlocked: night_sessions >= 1,
            current: night_sessions.min(1),
            target: 1,
        },
    ];

    for achievement in &mut achievements {
        let saved: Option<(i64, i64, Option<String>)> = connection
            .query_row(
                r#"
                SELECT current, unlocked, unlocked_at
                FROM profile_achievements
                WHERE profile_id = ?1 AND achievement_id = ?2
                "#,
                params![profile_id, achievement.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| format!("Could not load saved achievement progress: {error}"))?;

        let saved_current = saved.as_ref().map(|value| value.0).unwrap_or(0);
        let saved_unlocked = saved.as_ref().map(|value| value.1 != 0).unwrap_or(false);
        let saved_unlocked_at = saved.and_then(|value| value.2);

        achievement.current = achievement.current.max(saved_current).min(achievement.target);
        achievement.unlocked =
            achievement.unlocked || saved_unlocked || achievement.current >= achievement.target;

        let unlocked_at = if achievement.unlocked {
            saved_unlocked_at.or_else(|| Some(now()))
        } else {
            None
        };

        connection
            .execute(
                r#"
                INSERT INTO profile_achievements (
                    profile_id, achievement_id, current, unlocked, unlocked_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(profile_id, achievement_id) DO UPDATE SET
                    current = MAX(profile_achievements.current, excluded.current),
                    unlocked = MAX(profile_achievements.unlocked, excluded.unlocked),
                    unlocked_at = COALESCE(profile_achievements.unlocked_at, excluded.unlocked_at),
                    updated_at = excluded.updated_at
                "#,
                params![
                    profile_id,
                    achievement.id,
                    achievement.current,
                    if achievement.unlocked { 1_i64 } else { 0_i64 },
                    unlocked_at,
                    now()
                ],
            )
            .map_err(|error| format!("Could not save achievement progress: {error}"))?;
    }

    Ok(achievements)
}


fn validate_save_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err("The selected save folder does not exist.".into());
    }
    if !path.is_dir() {
        return Err("The selected save path is not a folder.".into());
    }

    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Could not resolve save folder: {error}"))?;

    if canonical.file_name().is_none() {
        return Err("A drive root cannot be used as a save folder.".into());
    }

    let protected_roots = [
        env::var("USERPROFILE").ok(),
        env::var("WINDIR").ok(),
        env::var("PROGRAMFILES").ok(),
        env::var("PROGRAMFILES(X86)").ok(),
        env::var("APPDATA").ok(),
        env::var("LOCALAPPDATA").ok(),
    ];

    for protected in protected_roots.into_iter().flatten() {
        if let Ok(protected) = PathBuf::from(protected).canonicalize() {
            if canonical == protected {
                return Err(
                    "Choose the game's specific save folder, not a broad system or profile folder."
                        .into(),
                );
            }
        }
    }

    Ok(canonical)
}

fn copy_directory_tree(source: &Path, destination: &Path) -> Result<(i64, i64), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("Could not create backup directory: {error}"))?;

    let mut file_count = 0_i64;
    let mut total_bytes = 0_i64;

    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let source_path = entry.path();
        let relative = source_path
            .strip_prefix(source)
            .map_err(|error| format!("Could not map save path: {error}"))?;

        if relative.as_os_str().is_empty() {
            continue;
        }

        let target = destination.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| format!("Could not create backup folder: {error}"))?;
            continue;
        }

        if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Could not create backup folder: {error}"))?;
            }

            let copied = fs::copy(source_path, &target)
                .map_err(|error| format!("Could not copy save file: {error}"))?;

            file_count += 1;
            total_bytes += copied as i64;
        }
    }

    Ok((file_count, total_bytes))
}

fn clear_directory_contents(path: &Path) -> Result<(), String> {
    let entries = fs::read_dir(path)
        .map_err(|error| format!("Could not read configured save folder: {error}"))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not inspect save folder: {error}"))?;
        let target = entry.path();

        if target.is_dir() {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("Could not clear save folder: {error}"))?;
        } else {
            fs::remove_file(&target)
                .map_err(|error| format!("Could not clear save file: {error}"))?;
        }
    }

    Ok(())
}

fn save_config_for_game(connection: &Connection, game_id: &str) -> Result<SaveConfig, String> {
    let profile_id = active_profile_id(connection)?;
    connection
        .query_row(
            "SELECT profile_id, game_id, save_path, configured_at FROM profile_save_configs WHERE profile_id = ?1 AND game_id = ?2",
            params![profile_id, game_id],
            |row| {
                Ok(SaveConfig {
                    profile_id: row.get(0)?,
                    game_id: row.get(1)?,
                    save_path: row.get(2)?,
                    configured_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Could not read save configuration: {error}"))?
        .ok_or_else(|| "No save folder is configured for this game.".to_string())
}

fn profile_vault_segment(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();

    if cleaned.is_empty() {
        "item".into()
    } else {
        cleaned
    }
}

fn profile_save_vault_dir(
    app: &AppHandle,
    profile_id: &str,
    game_id: &str,
) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?
        .join("profile-saves")
        .join(profile_vault_segment(profile_id))
        .join(profile_vault_segment(game_id)))
}

fn write_profile_vault_metadata(vault: &Path, profile_id: &str, game_id: &str) {
    if fs::create_dir_all(vault).is_err() {
        return;
    }

    let metadata = serde_json::json!({
        "schemaVersion": 1,
        "profileId": profile_id,
        "gameId": game_id,
        "updatedAt": now()
    });

    if let Ok(bytes) = serde_json::to_vec_pretty(&metadata) {
        let _ = fs::write(vault.join(".dusk-profile.json"), bytes);
    }
}

fn profile_vault_state(
    app: &AppHandle,
    profile_id: &str,
    game_id: &str,
) -> Result<ProfileSaveFileState, String> {
    let vault = profile_save_vault_dir(app, profile_id, game_id)?;
    if !vault.is_dir() {
        return Ok(ProfileSaveFileState {
            profile_id: profile_id.to_string(),
            game_id: game_id.to_string(),
            vault_path: vault.to_string_lossy().into_owned(),
            exists: false,
            file_count: 0,
            total_bytes: 0,
            updated_at: None,
        });
    }

    let mut file_count = 0_i64;
    let mut total_bytes = 0_i64;
    let mut newest: Option<std::time::SystemTime> = None;

    for entry in WalkDir::new(&vault)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        if entry.file_name().to_string_lossy() == ".dusk-profile.json" {
            continue;
        }

        file_count += 1;
        if let Ok(metadata) = entry.metadata() {
            total_bytes = total_bytes.saturating_add(metadata.len() as i64);
            if let Ok(modified) = metadata.modified() {
                if newest.map(|current| modified > current).unwrap_or(true) {
                    newest = Some(modified);
                }
            }
        }
    }

    let updated_at = newest.map(|modified| DateTime::<Utc>::from(modified).to_rfc3339());

    Ok(ProfileSaveFileState {
        profile_id: profile_id.to_string(),
        game_id: game_id.to_string(),
        vault_path: vault.to_string_lossy().into_owned(),
        exists: true,
        file_count,
        total_bytes,
        updated_at,
    })
}

fn create_save_backup_internal(
    app: &AppHandle,
    connection: &Connection,
    game_id: &str,
    kind: &str,
) -> Result<SaveBackupRecord, String> {
    let config = save_config_for_game(connection, game_id)?;
    let source = validate_save_directory(Path::new(&config.save_path))?;

    let id = Uuid::new_v4().to_string();
    let backup_root = app_data_dir(app)?
        .join("save-backups")
        .join(&config.profile_id)
        .join(game_id.replace(':', "_"))
        .join(&id);

    let (file_count, total_bytes) = copy_directory_tree(&source, &backup_root)?;
    let created_at = now();

    connection
        .execute(
            r#"
            INSERT INTO profile_save_backups
                (id, profile_id, game_id, backup_path, created_at, file_count, total_bytes, kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                id,
                config.profile_id,
                game_id,
                backup_root.to_string_lossy().into_owned(),
                created_at,
                file_count,
                total_bytes,
                kind
            ],
        )
        .map_err(|error| format!("Could not record save backup: {error}"))?;

    // Keep the owner's current vault in sync when they explicitly make a backup.
    if kind == "manual" {
        if let Ok(vault) = profile_save_vault_dir(app, &config.profile_id, game_id) {
            if vault.exists() {
                let _ = fs::remove_dir_all(&vault);
            }
            if copy_directory_tree(&source, &vault).is_ok() {
                write_profile_vault_metadata(&vault, &config.profile_id, game_id);
            }
        }
    }

    Ok(SaveBackupRecord {
        id,
        profile_id: config.profile_id,
        game_id: game_id.to_string(),
        backup_path: backup_root.to_string_lossy().into_owned(),
        created_at,
        file_count,
        total_bytes,
        kind: kind.to_string(),
    })
}

fn save_profile_files_blocking(
    app: AppHandle,
    game_id: String,
) -> Result<ProfileSaveFileState, String> {
    let connection = open_database(&app)?;
    let config = save_config_for_game(&connection, &game_id)?;
    let live = validate_save_directory(Path::new(&config.save_path))?;
    let vault = profile_save_vault_dir(&app, &config.profile_id, &game_id)?;

    if vault.exists() {
        fs::remove_dir_all(&vault)
            .map_err(|error| format!("Could not replace profile save files: {error}"))?;
    }
    copy_directory_tree(&live, &vault)?;
    write_profile_vault_metadata(&vault, &config.profile_id, &game_id);

    profile_vault_state(&app, &config.profile_id, &game_id)
}

#[tauri::command]
async fn save_profile_files(
    app: AppHandle,
    game_id: String,
) -> Result<ProfileSaveFileState, String> {
    tauri::async_runtime::spawn_blocking(move || save_profile_files_blocking(app, game_id))
        .await
        .map_err(|error| format!("Profile save worker failed: {error}"))?
}

fn load_profile_files_blocking(
    app: AppHandle,
    game_id: String,
) -> Result<ProfileSaveFileState, String> {
    let connection = open_database(&app)?;
    let config = save_config_for_game(&connection, &game_id)?;
    let live = validate_save_directory(Path::new(&config.save_path))?;
    let vault = profile_save_vault_dir(&app, &config.profile_id, &game_id)?;

    if !vault.is_dir() {
        return Err("This owner does not have saved profile files for this game yet.".into());
    }

    // Preserve the current live files before replacing them.
    let _ = create_save_backup_internal(&app, &connection, &game_id, "pre-profile-load")?;

    clear_directory_contents(&live)?;
    copy_directory_tree(&vault, &live)?;

    profile_vault_state(&app, &config.profile_id, &game_id)
}

#[tauri::command]
async fn load_profile_files(
    app: AppHandle,
    game_id: String,
) -> Result<ProfileSaveFileState, String> {
    tauri::async_runtime::spawn_blocking(move || load_profile_files_blocking(app, game_id))
        .await
        .map_err(|error| format!("Profile load worker failed: {error}"))?
}

#[tauri::command]
fn get_profile_save_file_state(
    app: AppHandle,
    game_id: String,
) -> Result<ProfileSaveFileState, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    profile_vault_state(&app, &profile_id, &game_id)
}

#[tauri::command]
fn choose_save_folder(app: AppHandle, game_id: String) -> Result<Option<SaveConfig>, String> {
    // Ensure the game exists before associating a filesystem location with it.
    let connection = open_database(&app)?;
    let _ = get_game(&connection, &game_id)?;

    let Some(selected) = rfd::FileDialog::new().pick_folder() else {
        return Ok(None);
    };

    let save_path = validate_save_directory(&selected)?;
    let configured_at = now();

    let profile_id = active_profile_id(&connection)?;
    connection
        .execute(
            r#"
            INSERT INTO profile_save_configs (profile_id, game_id, save_path, configured_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(profile_id, game_id) DO UPDATE SET
                save_path = excluded.save_path,
                configured_at = excluded.configured_at
            "#,
            params![
                profile_id,
                game_id,
                save_path.to_string_lossy().into_owned(),
                configured_at
            ],
        )
        .map_err(|error| format!("Could not save backup configuration: {error}"))?;

    // Seed this owner's physical save vault with the current live files.
    let vault = profile_save_vault_dir(&app, &profile_id, &game_id)?;
    if vault.exists() {
        let _ = fs::remove_dir_all(&vault);
    }
    let _ = copy_directory_tree(&save_path, &vault);
    write_profile_vault_metadata(&vault, &profile_id, &game_id);

    Ok(Some(SaveConfig {
        profile_id,
        game_id,
        save_path: save_path.to_string_lossy().into_owned(),
        configured_at,
    }))
}

#[tauri::command]
fn get_save_config(app: AppHandle, game_id: String) -> Result<Option<SaveConfig>, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    connection
        .query_row(
            "SELECT profile_id, game_id, save_path, configured_at FROM profile_save_configs WHERE profile_id = ?1 AND game_id = ?2",
            params![profile_id, game_id],
            |row| {
                Ok(SaveConfig {
                    profile_id: row.get(0)?,
                    game_id: row.get(1)?,
                    save_path: row.get(2)?,
                    configured_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Could not load save configuration: {error}"))
}

#[tauri::command]
fn clear_save_config(app: AppHandle, game_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    connection
        .execute(
            "DELETE FROM profile_save_configs WHERE profile_id = ?1 AND game_id = ?2",
            params![profile_id, game_id],
        )
        .map_err(|error| format!("Could not clear save configuration: {error}"))?;
    Ok(())
}

fn create_save_backup_blocking(app: AppHandle, game_id: String) -> Result<SaveBackupRecord, String> {
    let connection = open_database(&app)?;
    create_save_backup_internal(&app, &connection, &game_id, "manual")
}

#[tauri::command]
async fn create_save_backup(app: AppHandle, game_id: String) -> Result<SaveBackupRecord, String> {
    tauri::async_runtime::spawn_blocking(move || create_save_backup_blocking(app, game_id))
        .await
        .map_err(|error| format!("Save backup worker failed: {error}"))?
}

#[tauri::command]
fn list_save_backups(app: AppHandle, game_id: String) -> Result<Vec<SaveBackupRecord>, String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, profile_id, game_id, backup_path, created_at, file_count, total_bytes, kind
            FROM profile_save_backups
            WHERE profile_id = ?1 AND game_id = ?2
            ORDER BY created_at DESC
            "#,
        )
        .map_err(|error| format!("Could not prepare backup list: {error}"))?;

    let rows = statement
        .query_map(params![profile_id, game_id], |row| {
            Ok(SaveBackupRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                game_id: row.get(2)?,
                backup_path: row.get(3)?,
                created_at: row.get(4)?,
                file_count: row.get(5)?,
                total_bytes: row.get(6)?,
                kind: row.get(7)?,
            })
        })
        .map_err(|error| format!("Could not load save backups: {error}"))?;

    let mut backups = Vec::new();
    for row in rows {
        backups.push(row.map_err(|error| format!("Could not decode save backup: {error}"))?);
    }
    Ok(backups)
}

fn restore_save_backup_blocking(
    app: AppHandle,
    game_id: String,
    backup_id: String,
) -> Result<SaveBackupRecord, String> {
    let connection = open_database(&app)?;
    let config = save_config_for_game(&connection, &game_id)?;
    let destination = validate_save_directory(Path::new(&config.save_path))?;

    let profile_id = active_profile_id(&connection)?;
    let backup_path: String = connection
        .query_row(
            "SELECT backup_path FROM profile_save_backups WHERE id = ?1 AND profile_id = ?2 AND game_id = ?3",
            params![backup_id, profile_id, game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not load save backup: {error}"))?
        .ok_or_else(|| "Backup not found.".to_string())?;

    let source = PathBuf::from(backup_path);
    if !source.is_dir() {
        return Err("The backup files are missing from disk.".into());
    }

    // Always preserve the current saves before a destructive restore.
    let safety_backup = create_save_backup_internal(&app, &connection, &game_id, "pre-restore")?;

    clear_directory_contents(&destination)?;
    if let Err(error) = copy_directory_tree(&source, &destination) {
        return Err(format!(
            "Restore failed after creating safety backup {}: {}",
            safety_backup.id, error
        ));
    }

    Ok(safety_backup)
}

#[tauri::command]
async fn restore_save_backup(
    app: AppHandle,
    game_id: String,
    backup_id: String,
) -> Result<SaveBackupRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        restore_save_backup_blocking(app, game_id, backup_id)
    })
    .await
    .map_err(|error| format!("Save restore worker failed: {error}"))?
}

#[tauri::command]
fn delete_save_backup(app: AppHandle, game_id: String, backup_id: String) -> Result<(), String> {
    let connection = open_database(&app)?;
    let profile_id = active_profile_id(&connection)?;
    let backup_path: Option<String> = connection
        .query_row(
            "SELECT backup_path FROM profile_save_backups WHERE id = ?1 AND profile_id = ?2 AND game_id = ?3",
            params![backup_id, profile_id, game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not locate save backup: {error}"))?;

    if let Some(backup_path) = backup_path {
        let path = PathBuf::from(backup_path);
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("Could not delete backup files: {error}"))?;
        }
    }

    connection
        .execute(
            "DELETE FROM profile_save_backups WHERE id = ?1 AND profile_id = ?2 AND game_id = ?3",
            params![backup_id, profile_id, game_id],
        )
        .map_err(|error| format!("Could not delete backup record: {error}"))?;

    Ok(())
}


fn safe_storage_segment(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();

    if cleaned.is_empty() {
        "item".into()
    } else {
        cleaned
    }
}

fn validate_cloud_identity(
    supabase_url: &str,
    publishable_key: &str,
    access_token: &str,
    auth_user_id: &str,
) -> Result<reqwest::Url, String> {
    if publishable_key.trim().is_empty() || access_token.trim().is_empty() {
        return Err("Supabase cloud credentials are missing.".into());
    }

    Uuid::parse_str(auth_user_id)
        .map_err(|_| "Supabase cloud user ID is invalid.".to_string())?;

    let url = reqwest::Url::parse(supabase_url.trim())
        .map_err(|error| format!("Supabase project URL is invalid: {error}"))?;

    if url.scheme() != "https" {
        return Err("Supabase cloud saves require an HTTPS project URL.".into());
    }

    Ok(url)
}

fn storage_object_url(
    supabase_url: &reqwest::Url,
    segments: &[String],
) -> Result<reqwest::Url, String> {
    let mut url = supabase_url
        .join("storage/v1/object/")
        .map_err(|error| format!("Could not build Supabase Storage URL: {error}"))?;

    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| "Supabase Storage URL cannot contain path segments.".to_string())?;
        path.push("dusk-savefiles");
        for segment in segments {
            path.push(segment);
        }
    }

    Ok(url)
}

fn upload_storage_object(
    client: &reqwest::blocking::Client,
    supabase_url: &reqwest::Url,
    publishable_key: &str,
    access_token: &str,
    segments: &[String],
    content_type: &str,
    body: Vec<u8>,
) -> Result<(), String> {
    let url = storage_object_url(supabase_url, segments)?;
    let response = client
        .post(url)
        .header("apikey", publishable_key)
        .bearer_auth(access_token)
        .header("x-upsert", "true")
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(body)
        .send()
        .map_err(|error| format!("Supabase upload failed: {error}"))?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().unwrap_or_default();
    Err(format!(
        "Supabase Storage rejected an upload ({status}): {}",
        body.chars().take(300).collect::<String>()
    ))
}

fn upload_save_backup_to_cloud_blocking(
    app: AppHandle,
    backup_id: String,
    supabase_url: String,
    publishable_key: String,
    access_token: String,
    auth_user_id: String,
) -> Result<CloudUploadResult, String> {
    let supabase_url = validate_cloud_identity(
        &supabase_url,
        &publishable_key,
        &access_token,
        &auth_user_id,
    )?;

    let connection = open_database(&app)?;

    let backup: SaveBackupRecord = connection
        .query_row(
            r#"
            SELECT id, profile_id, game_id, backup_path, created_at, file_count, total_bytes, kind
            FROM profile_save_backups
            WHERE id = ?1
            "#,
            params![backup_id],
            |row| {
                Ok(SaveBackupRecord {
                    id: row.get(0)?,
                    profile_id: row.get(1)?,
                    game_id: row.get(2)?,
                    backup_path: row.get(3)?,
                    created_at: row.get(4)?,
                    file_count: row.get(5)?,
                    total_bytes: row.get(6)?,
                    kind: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Could not load backup for cloud sync: {error}"))?
        .ok_or_else(|| "Save backup not found.".to_string())?;

    let backup_root = PathBuf::from(&backup.backup_path);
    if !backup_root.is_dir() {
        return Err("The local backup files are missing.".into());
    }

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| format!("Could not initialize cloud client: {error}"))?;

    let base_segments = vec![
        auth_user_id.clone(),
        "profiles".into(),
        backup.profile_id.clone(),
        "games".into(),
        backup.game_id.clone(),
        "backups".into(),
        backup.id.clone(),
    ];

    let mut uploaded_files = 0_usize;
    let mut uploaded_bytes = 0_u64;

    for entry in WalkDir::new(&backup_root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        if uploaded_files >= 10_000 {
            return Err("Cloud backup stopped because it exceeded 10,000 save files.".into());
        }

        let relative = entry
            .path()
            .strip_prefix(&backup_root)
            .map_err(|error| format!("Could not map backup file for upload: {error}"))?;

        let mut segments = base_segments.clone();
        for component in relative.components() {
            match component {
                std::path::Component::Normal(segment) => {
                    segments.push(segment.to_string_lossy().into_owned());
                }
                _ => return Err("Unsafe save-file path encountered during cloud upload.".into()),
            }
        }

        let bytes = fs::read(entry.path())
            .map_err(|error| format!("Could not read save file for cloud upload: {error}"))?;
        uploaded_bytes = uploaded_bytes.saturating_add(bytes.len() as u64);

        upload_storage_object(
            &client,
            &supabase_url,
            &publishable_key,
            &access_token,
            &segments,
            "application/octet-stream",
            bytes,
        )?;

        uploaded_files += 1;
    }

    let metadata = serde_json::to_vec_pretty(&serde_json::json!({
        "backupId": backup.id,
        "profileId": backup.profile_id,
        "gameId": backup.game_id,
        "createdAt": backup.created_at,
        "fileCount": backup.file_count,
        "totalBytes": backup.total_bytes,
        "kind": backup.kind,
        "uploadedFiles": uploaded_files,
        "uploadedBytes": uploaded_bytes
    }))
    .map_err(|error| format!("Could not encode cloud backup metadata: {error}"))?;

    let mut metadata_segments = base_segments;
    metadata_segments.push("_backup.json".into());
    upload_storage_object(
        &client,
        &supabase_url,
        &publishable_key,
        &access_token,
        &metadata_segments,
        "application/json",
        metadata,
    )?;

    Ok(CloudUploadResult {
        backup_id: backup.id,
        profile_id: backup.profile_id,
        game_id: backup.game_id,
        uploaded_files,
        uploaded_bytes,
    })
}

#[tauri::command]
async fn upload_save_backup_to_cloud(
    app: AppHandle,
    backup_id: String,
    supabase_url: String,
    publishable_key: String,
    access_token: String,
    auth_user_id: String,
) -> Result<CloudUploadResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        upload_save_backup_to_cloud_blocking(
            app,
            backup_id,
            supabase_url,
            publishable_key,
            access_token,
            auth_user_id,
        )
    })
    .await
    .map_err(|error| format!("Cloud backup worker failed: {error}"))?
}


fn upload_all_save_backups_to_cloud_blocking(
    app: AppHandle,
    supabase_url: String,
    publishable_key: String,
    access_token: String,
    auth_user_id: String,
) -> Result<CloudSyncAllResult, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare("SELECT id FROM profile_save_backups ORDER BY created_at ASC")
        .map_err(|error| format!("Could not prepare cloud backup list: {error}"))?;

    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not load cloud backup list: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode cloud backup list: {error}"))?;

    drop(statement);
    drop(connection);

    let mut uploaded_files = 0_usize;
    let mut uploaded_bytes = 0_u64;
    let mut backups = 0_usize;

    for backup_id in ids {
        let result = upload_save_backup_to_cloud_blocking(
            app.clone(),
            backup_id,
            supabase_url.clone(),
            publishable_key.clone(),
            access_token.clone(),
            auth_user_id.clone(),
        )?;
        backups += 1;
        uploaded_files = uploaded_files.saturating_add(result.uploaded_files);
        uploaded_bytes = uploaded_bytes.saturating_add(result.uploaded_bytes);
    }

    upload_cloud_manifest_blocking(
        app,
        supabase_url,
        publishable_key,
        access_token,
        auth_user_id,
    )?;

    Ok(CloudSyncAllResult {
        backups,
        uploaded_files,
        uploaded_bytes,
    })
}

#[tauri::command]
async fn upload_all_save_backups_to_cloud(
    app: AppHandle,
    supabase_url: String,
    publishable_key: String,
    access_token: String,
    auth_user_id: String,
) -> Result<CloudSyncAllResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        upload_all_save_backups_to_cloud_blocking(
            app,
            supabase_url,
            publishable_key,
            access_token,
            auth_user_id,
        )
    })
    .await
    .map_err(|error| format!("Cloud sync worker failed: {error}"))?
}



fn migrate_legacy_data_to_account(app: &AppHandle, account_dir: &Path) -> Result<(), String> {
    let base = base_app_data_dir(app)?;
    let marker = base.join(".legacy-account-adopted");
    if marker.exists() || account_dir.join("dusk.db").exists() || !base.join("dusk.db").exists() {
        return Ok(());
    }

    fs::create_dir_all(account_dir)
        .map_err(|error| format!("Could not create account data directory: {error}"))?;

    for filename in ["dusk.db", "dusk.db-wal", "dusk.db-shm"] {
        let source = base.join(filename);
        if source.is_file() {
            fs::copy(&source, account_dir.join(filename))
                .map_err(|error| format!("Could not migrate legacy {filename}: {error}"))?;
        }
    }

    for dirname in ["covers", "screenshots", "profile-saves", "save-backups"] {
        let source = base.join(dirname);
        if source.is_dir() {
            copy_directory_tree(&source, &account_dir.join(dirname))?;
        }
    }

    fs::write(&marker, "Dusk legacy data adopted by the first signed-in account.\n")
        .map_err(|error| format!("Could not mark legacy account migration: {error}"))?;
    Ok(())
}

#[tauri::command]
async fn set_account_scope(app: AppHandle, account_user_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        set_account_scope_blocking(app, account_user_id)
    })
    .await
    .map_err(|error| format!("Account scope worker failed: {error}"))?
}

fn set_account_scope_blocking(app: AppHandle, account_user_id: Option<String>) -> Result<(), String> {
    let normalized = match account_user_id {
        Some(value) => {
            let parsed = Uuid::parse_str(value.trim())
                .map_err(|_| "Invalid Dusk account user id.".to_string())?;
            Some(parsed.to_string())
        }
        None => None,
    };

    if let Some(user_id) = normalized.as_ref() {
        let account_dir = base_app_data_dir(&app)?.join("accounts").join(user_id);
        migrate_legacy_data_to_account(&app, &account_dir)?;
    }

    *ACCOUNT_SCOPE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "Could not update Dusk account scope.".to_string())? = normalized;

    if current_account_scope().is_some() {
        let _ = open_database(&app)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct CloudPlaytimeSession {
    device_id: String,
    session_id: i64,
    game_id: String,
    duration_seconds: i64,
    ended_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudPlaytimeUpload {
    baselines: Vec<serde_json::Value>,
    sessions: Vec<CloudPlaytimeSession>,
}
#[tauri::command]
async fn export_playtime_updates(app: AppHandle) -> Result<CloudPlaytimeUpload, String> {
    tauri::async_runtime::spawn_blocking(move || export_playtime_updates_blocking(app))
        .await
        .map_err(|error| format!("Playtime export worker failed: {error}"))?
}

fn export_playtime_updates_blocking(app: AppHandle) -> Result<CloudPlaytimeUpload, String> {
    let db = open_database(&app)?;
    let device_id: String = match db.query_row("SELECT value FROM app_settings WHERE key='playtime_device_id'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())? {
        Some(id) => id,
        None => {
            let id=Uuid::new_v4().to_string();
            db.execute("INSERT INTO app_settings(key,value) VALUES ('playtime_device_id',?1)",params![id]).map_err(|e|e.to_string())?;
            id
        }
    };
    let cutoff: i64 = match db.query_row("SELECT value FROM app_settings WHERE key='playtime_cutoff'",[],|r|r.get::<_,String>(0)).optional().map_err(|e|e.to_string())? {
        Some(value) => value.parse().unwrap_or(0),
        None => {
            let max_id: i64 = db.query_row("SELECT COALESCE(MAX(id),0) FROM sessions",[],|r|r.get(0)).map_err(|e|e.to_string())?;
            db.execute("INSERT INTO app_settings(key,value) VALUES ('playtime_cutoff',?1)",params![max_id.to_string()]).map_err(|e|e.to_string())?;
            max_id
        }
    };
    let mut baseline_stmt=db.prepare("SELECT id,total_seconds,launch_count,last_played FROM games WHERE total_seconds>0 OR launch_count>0").map_err(|e|e.to_string())?;
    let baselines=baseline_stmt.query_map([],|r|Ok(serde_json::json!({"game_id":r.get::<_,String>(0)?,"total_seconds":r.get::<_,i64>(1)?.max(0),"launch_count":r.get::<_,i64>(2)?.max(0),"last_played":r.get::<_,Option<String>>(3)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    let mut stmt=db.prepare("SELECT id,game_id,duration_seconds,ended_at FROM sessions WHERE id>?1 ORDER BY id").map_err(|e|e.to_string())?;
    let sessions=stmt.query_map(params![cutoff],|r|Ok(CloudPlaytimeSession{device_id:device_id.clone(),session_id:r.get(0)?,game_id:r.get(1)?,duration_seconds:r.get(2)?,ended_at:r.get(3)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    Ok(CloudPlaytimeUpload{baselines,sessions})
}
#[tauri::command]
async fn merge_cloud_playtime(app: AppHandle, totals: serde_json::Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || merge_cloud_playtime_blocking(app, totals))
        .await
        .map_err(|error| format!("Playtime merge worker failed: {error}"))?
}

fn merge_cloud_playtime_blocking(app: AppHandle, totals: serde_json::Value) -> Result<(), String> {
    let mut db=open_database(&app)?;
    let tx=db.transaction().map_err(|e|e.to_string())?;
    if let Some(entries)=totals.as_array() {
        for entry in entries {
            let Some(id)=entry.get("game_id").and_then(|v|v.as_str()) else {continue};
            let seconds=entry.get("total_seconds").and_then(|v|v.as_i64()).unwrap_or(0).max(0);
            let launches=entry.get("launch_count").and_then(|v|v.as_i64()).unwrap_or(0).max(0);
            let last=entry.get("last_played").and_then(|v|v.as_str());
            tx.execute("UPDATE games SET total_seconds=MAX(total_seconds,?2), launch_count=MAX(launch_count,?3), last_played=CASE WHEN ?4 > COALESCE(last_played,'') THEN ?4 ELSE last_played END WHERE id=?1",params![id,seconds,launches,last]).map_err(|e|e.to_string())?;
        }
    }
    tx.commit().map_err(|e|e.to_string())
}

#[tauri::command]
async fn export_account_state(app: AppHandle) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || export_account_state_blocking(app))
        .await
        .map_err(|error| format!("Account-state export worker failed: {error}"))?
}

fn export_account_state_blocking(app: AppHandle) -> Result<serde_json::Value, String> {
    let _ = list_achievements(app.clone())?;
    let connection = open_database(&app)?;
    let active_profile = active_profile_id(&connection)?;

    let mut profiles_statement = connection
        .prepare("SELECT id, name, created_at, last_used_at FROM profiles ORDER BY created_at ASC")
        .map_err(|error| format!("Could not prepare account profiles: {error}"))?;
    let profiles = profiles_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "createdAt": row.get::<_, String>(2)?,
                "lastUsedAt": row.get::<_, String>(3)?
            }))
        })
        .map_err(|error| format!("Could not read account profiles: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode account profiles: {error}"))?;

    let mut games_statement = connection
        .prepare(
            r#"
            SELECT id, title, source, source_id, favorite, added_at, last_played, total_seconds, launch_count
            FROM games
            ORDER BY added_at ASC
            "#,
        )
        .map_err(|error| format!("Could not prepare account games: {error}"))?;
    let games = games_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "source": row.get::<_, String>(2)?,
                "sourceId": row.get::<_, Option<String>>(3)?,
                "favorite": row.get::<_, i64>(4)? != 0,
                "addedAt": row.get::<_, String>(5)?,
                "lastPlayed": row.get::<_, Option<String>>(6)?,
                "totalSeconds": row.get::<_, i64>(7)?,
                "launchCount": row.get::<_, i64>(8)?
            }))
        })
        .map_err(|error| format!("Could not read account games: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode account games: {error}"))?;

    let mut profile_games_statement = connection
        .prepare("SELECT profile_id, game_id, added_at FROM profile_games ORDER BY profile_id, added_at")
        .map_err(|error| format!("Could not prepare account profile games: {error}"))?;
    let profile_games = profile_games_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "profileId": row.get::<_, String>(0)?,
                "gameId": row.get::<_, String>(1)?,
                "addedAt": row.get::<_, String>(2)?
            }))
        })
        .map_err(|error| format!("Could not read account profile games: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode account profile games: {error}"))?;

    let mut collections_statement = connection
        .prepare("SELECT id, name, created_at FROM collections ORDER BY created_at ASC")
        .map_err(|error| format!("Could not prepare account collections: {error}"))?;
    let collections = collections_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "createdAt": row.get::<_, String>(2)?
            }))
        })
        .map_err(|error| format!("Could not read account collections: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode account collections: {error}"))?;

    let mut memberships_statement = connection
        .prepare("SELECT collection_id, game_id FROM collection_games ORDER BY collection_id, game_id")
        .map_err(|error| format!("Could not prepare account collection memberships: {error}"))?;
    let collection_games = memberships_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "collectionId": row.get::<_, String>(0)?,
                "gameId": row.get::<_, String>(1)?
            }))
        })
        .map_err(|error| format!("Could not read account collection memberships: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode account collection memberships: {error}"))?;

    let mut achievements_statement = connection
        .prepare(
            r#"
            SELECT profile_id, achievement_id, current, unlocked, unlocked_at, updated_at
            FROM profile_achievements
            ORDER BY profile_id, achievement_id
            "#,
        )
        .map_err(|error| format!("Could not prepare account achievements: {error}"))?;
    let profile_achievements = achievements_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "profileId": row.get::<_, String>(0)?,
                "achievementId": row.get::<_, String>(1)?,
                "current": row.get::<_, i64>(2)?,
                "unlocked": row.get::<_, i64>(3)? != 0,
                "unlockedAt": row.get::<_, Option<String>>(4)?,
                "updatedAt": row.get::<_, String>(5)?
            }))
        })
        .map_err(|error| format!("Could not read account achievements: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode account achievements: {error}"))?;

    Ok(serde_json::json!({
        "schemaVersion": 2,
        "generatedAt": now(),
        "activeProfileId": active_profile,
        "profiles": profiles,
        "games": games,
        "profileGames": profile_games,
        "collections": collections,
        "collectionGames": collection_games,
        "profileAchievements": profile_achievements
    }))
}

#[tauri::command]
async fn import_account_state(app: AppHandle, state: serde_json::Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || import_account_state_blocking(app, state))
        .await
        .map_err(|error| format!("Account-state import worker failed: {error}"))?
}

fn import_account_state_blocking(app: AppHandle, state: serde_json::Value) -> Result<(), String> {
    let connection = open_database(&app)?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| format!("Could not start account-state merge: {error}"))?;

    if let Some(profiles) = state.get("profiles").and_then(|value| value.as_array()) {
        for profile in profiles {
            let Some(id) = profile.get("id").and_then(|value| value.as_str()) else { continue; };
            let Some(name) = profile.get("name").and_then(|value| value.as_str()) else { continue; };
            let created_at = profile.get("createdAt").and_then(|value| value.as_str()).unwrap_or_else(|| "");
            let last_used_at = profile.get("lastUsedAt").and_then(|value| value.as_str()).unwrap_or(created_at);
            if created_at.is_empty() { continue; }

            transaction
                .execute(
                    r#"
                    INSERT INTO profiles (id, name, created_at, last_used_at)
                    VALUES (?1, ?2, ?3, ?4)
                    ON CONFLICT(id) DO UPDATE SET
                      name = excluded.name,
                      last_used_at = excluded.last_used_at
                    "#,
                    params![id, name, created_at, last_used_at],
                )
                .map_err(|error| format!("Could not merge account profile: {error}"))?;
        }
    }

    if let Some(games) = state.get("games").and_then(|value| value.as_array()) {
        for game in games {
            let Some(id) = game.get("id").and_then(|value| value.as_str()) else { continue; };
            let Some(title) = game.get("title").and_then(|value| value.as_str()) else { continue; };
            let source = game.get("source").and_then(|value| value.as_str()).unwrap_or("cloud");
            let source_id = game.get("sourceId").and_then(|value| value.as_str());
            let favorite = if game.get("favorite").and_then(|value| value.as_bool()).unwrap_or(false) { 1_i64 } else { 0_i64 };
            let added_at = game.get("addedAt").and_then(|value| value.as_str()).unwrap_or_else(|| "");
            if added_at.is_empty() { continue; }
            let last_played = game.get("lastPlayed").and_then(|value| value.as_str());
            let total_seconds = game.get("totalSeconds").and_then(|value| value.as_i64()).unwrap_or(0);
            let launch_count = game.get("launchCount").and_then(|value| value.as_i64()).unwrap_or(0);

            transaction
                .execute(
                    r#"
                    INSERT INTO games (
                      id, title, exe_path, install_path, source, source_id, favorite,
                      hidden, cover_path, added_at, last_played, total_seconds, launch_count
                    )
                    VALUES (?1, ?2, NULL, '', ?3, ?4, ?5, 0, NULL, ?6, ?7, ?8, ?9)
                    ON CONFLICT(id) DO UPDATE SET
                      title = excluded.title,
                      source = excluded.source,
                      source_id = excluded.source_id,
                      favorite = excluded.favorite,
                      last_played = COALESCE(excluded.last_played, games.last_played),
                      total_seconds = MAX(games.total_seconds, excluded.total_seconds),
                      launch_count = MAX(games.launch_count, excluded.launch_count)
                    "#,
                    params![
                        id,
                        title,
                        source,
                        source_id,
                        favorite,
                        added_at,
                        last_played,
                        total_seconds,
                        launch_count
                    ],
                )
                .map_err(|error| format!("Could not merge account game: {error}"))?;
        }
    }

    if let Some(profile_games) = state.get("profileGames").and_then(|value| value.as_array()) {
        for link in profile_games {
            let Some(profile_id) = link.get("profileId").and_then(|value| value.as_str()) else { continue; };
            let Some(game_id) = link.get("gameId").and_then(|value| value.as_str()) else { continue; };
            let added_at = link.get("addedAt").and_then(|value| value.as_str()).unwrap_or_else(|| "");
            if added_at.is_empty() { continue; }
            transaction
                .execute(
                    "INSERT OR IGNORE INTO profile_games (profile_id, game_id, added_at) VALUES (?1, ?2, ?3)",
                    params![profile_id, game_id, added_at],
                )
                .map_err(|error| format!("Could not merge account library membership: {error}"))?;
        }
    }

    if let Some(collections) = state.get("collections").and_then(|value| value.as_array()) {
        for collection in collections {
            let Some(id) = collection.get("id").and_then(|value| value.as_str()) else { continue; };
            let Some(name) = collection.get("name").and_then(|value| value.as_str()) else { continue; };
            let created_at = collection.get("createdAt").and_then(|value| value.as_str()).unwrap_or_else(|| "");
            if created_at.is_empty() { continue; }
            transaction
                .execute(
                    "INSERT OR IGNORE INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
                    params![id, name, created_at],
                )
                .map_err(|error| format!("Could not merge account collection: {error}"))?;
        }
    }

    if let Some(collection_games) = state.get("collectionGames").and_then(|value| value.as_array()) {
        for link in collection_games {
            let Some(collection_id) = link.get("collectionId").and_then(|value| value.as_str()) else { continue; };
            let Some(game_id) = link.get("gameId").and_then(|value| value.as_str()) else { continue; };
            let _ = transaction.execute(
                "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
                params![collection_id, game_id],
            );
        }
    }

    if let Some(achievements) = state.get("profileAchievements").and_then(|value| value.as_array()) {
        for achievement in achievements {
            let Some(profile_id) = achievement.get("profileId").and_then(|value| value.as_str()) else { continue; };
            let Some(achievement_id) = achievement.get("achievementId").and_then(|value| value.as_str()) else { continue; };
            let current = achievement.get("current").and_then(|value| value.as_i64()).unwrap_or(0).max(0);
            let unlocked = if achievement.get("unlocked").and_then(|value| value.as_bool()).unwrap_or(false) {
                1_i64
            } else {
                0_i64
            };
            let unlocked_at = achievement.get("unlockedAt").and_then(|value| value.as_str());
            let updated_at = achievement
                .get("updatedAt")
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| "");

            let profile_exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = ?1)",
                    params![profile_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !profile_exists {
                continue;
            }

            transaction
                .execute(
                    r#"
                    INSERT INTO profile_achievements (
                        profile_id, achievement_id, current, unlocked, unlocked_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ON CONFLICT(profile_id, achievement_id) DO UPDATE SET
                        current = MAX(profile_achievements.current, excluded.current),
                        unlocked = MAX(profile_achievements.unlocked, excluded.unlocked),
                        unlocked_at = COALESCE(profile_achievements.unlocked_at, excluded.unlocked_at),
                        updated_at = CASE
                            WHEN excluded.current > profile_achievements.current
                              OR excluded.unlocked > profile_achievements.unlocked
                            THEN excluded.updated_at
                            ELSE profile_achievements.updated_at
                        END
                    "#,
                    params![
                        profile_id,
                        achievement_id,
                        current,
                        unlocked,
                        unlocked_at,
                        if updated_at.is_empty() { now() } else { updated_at.to_string() }
                    ],
                )
                .map_err(|error| format!("Could not merge account achievement: {error}"))?;
        }
    }

    if let Some(active_profile) = state.get("activeProfileId").and_then(|value| value.as_str()) {
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = ?1)",
                params![active_profile],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_profile_id', ?1)",
                    params![active_profile],
                )
                .map_err(|error| format!("Could not restore active profile: {error}"))?;
        }
    }

    transaction
        .commit()
        .map_err(|error| format!("Could not commit account-state merge: {error}"))?;
    Ok(())
}

fn upload_cloud_manifest_blocking(
    app: AppHandle,
    supabase_url: String,
    publishable_key: String,
    access_token: String,
    auth_user_id: String,
) -> Result<(), String> {
    let supabase_url = validate_cloud_identity(
        &supabase_url,
        &publishable_key,
        &access_token,
        &auth_user_id,
    )?;
    let connection = open_database(&app)?;

    let mut profiles_statement = connection
        .prepare(
            "SELECT id, name, created_at, last_used_at FROM profiles ORDER BY last_used_at DESC",
        )
        .map_err(|error| format!("Could not prepare cloud profile manifest: {error}"))?;

    let profile_rows = profiles_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "createdAt": row.get::<_, String>(2)?,
                "lastUsedAt": row.get::<_, String>(3)?
            }))
        })
        .map_err(|error| format!("Could not load cloud profile manifest: {error}"))?;

    let profiles = profile_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode cloud profile manifest: {error}"))?;

    let mut games_statement = connection
        .prepare(
            r#"
            SELECT pg.profile_id, g.id, g.title, g.source, pg.added_at
            FROM profile_games pg
            INNER JOIN games g ON g.id = pg.game_id
            ORDER BY pg.profile_id, LOWER(g.title)
            "#,
        )
        .map_err(|error| format!("Could not prepare cloud game manifest: {error}"))?;

    let game_rows = games_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "profileId": row.get::<_, String>(0)?,
                "gameId": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "source": row.get::<_, String>(3)?,
                "addedAt": row.get::<_, String>(4)?
            }))
        })
        .map_err(|error| format!("Could not load cloud game manifest: {error}"))?;

    let games = game_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode cloud game manifest: {error}"))?;

    let mut backups_statement = connection
        .prepare(
            r#"
            SELECT id, profile_id, game_id, created_at, file_count, total_bytes, kind
            FROM profile_save_backups
            ORDER BY profile_id, game_id, created_at DESC
            "#,
        )
        .map_err(|error| format!("Could not prepare cloud backup manifest: {error}"))?;

    let backup_rows = backups_statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "profileId": row.get::<_, String>(1)?,
                "gameId": row.get::<_, String>(2)?,
                "createdAt": row.get::<_, String>(3)?,
                "fileCount": row.get::<_, i64>(4)?,
                "totalBytes": row.get::<_, i64>(5)?,
                "kind": row.get::<_, String>(6)?
            }))
        })
        .map_err(|error| format!("Could not load cloud backup manifest: {error}"))?;

    let backups = backup_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode cloud backup manifest: {error}"))?;

    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaVersion": 1,
        "generatedAt": now(),
        "profiles": profiles,
        "games": games,
        "backups": backups
    }))
    .map_err(|error| format!("Could not encode cloud manifest: {error}"))?;

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("Could not initialize cloud client: {error}"))?;

    upload_storage_object(
        &client,
        &supabase_url,
        &publishable_key,
        &access_token,
        &[auth_user_id, "manifest.json".into()],
        "application/json",
        manifest,
    )
}

#[tauri::command]
async fn upload_cloud_manifest(
    app: AppHandle,
    supabase_url: String,
    publishable_key: String,
    access_token: String,
    auth_user_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        upload_cloud_manifest_blocking(
            app,
            supabase_url,
            publishable_key,
            access_token,
            auth_user_id,
        )
    })
    .await
    .map_err(|error| format!("Cloud manifest worker failed: {error}"))?
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuskUpdateInfo {
    available: bool,
    version: Option<String>,
    body: Option<String>,
    date: Option<String>,
}

fn normalized_version_parts(value: &str) -> Vec<u64> {
    let clean = value
        .trim()
        .trim_start_matches("dusk-v")
        .trim_start_matches('v');

    let mut parts = clean
        .split('.')
        .take(3)
        .map(|part| {
            let digits: String = part.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            digits.parse::<u64>().unwrap_or(0)
        })
        .collect::<Vec<_>>();

    while parts.len() < 3 {
        parts.push(0);
    }
    parts
}

fn version_is_newer(remote: &str, local: &str) -> bool {
    normalized_version_parts(remote) > normalized_version_parts(local)
}

fn github_release_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| format!("Could not initialize Dusk updater: {error}"))
}

fn fetch_latest_dusk_release(
    client: &reqwest::blocking::Client,
) -> Result<serde_json::Value, String> {
    client
        .get("https://api.github.com/repos/bxanedot/dusk/releases/latest")
        .header(reqwest::header::USER_AGENT, "Dusk-Desktop-Updater")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("Could not check GitHub Releases: {error}"))?
        .json::<serde_json::Value>()
        .map_err(|error| format!("Could not decode the latest Dusk release: {error}"))
}

fn release_version(release: &serde_json::Value) -> Result<String, String> {
    release
        .get("tag_name")
        .and_then(|value| value.as_str())
        .map(|tag| tag.trim_start_matches("dusk-v").trim_start_matches('v').to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Latest Dusk release is missing a version tag.".to_string())
}

fn check_github_update_blocking() -> Result<DuskUpdateInfo, String> {
    let client = github_release_client()?;
    let release = fetch_latest_dusk_release(&client)?;
    let version = release_version(&release)?;
    let current = env!("CARGO_PKG_VERSION");

    Ok(DuskUpdateInfo {
        available: version_is_newer(&version, current),
        version: Some(version),
        body: release.get("body").and_then(|value| value.as_str()).map(str::to_string),
        date: release
            .get("published_at")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

#[tauri::command]
async fn check_github_update() -> Result<DuskUpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(check_github_update_blocking)
        .await
        .map_err(|error| format!("Dusk updater worker failed: {error}"))?
}

fn download_latest_dusk_installer() -> Result<PathBuf, String> {
    let client = github_release_client()?;
    let release = fetch_latest_dusk_release(&client)?;
    let version = release_version(&release)?;

    if !version_is_newer(&version, env!("CARGO_PKG_VERSION")) {
        return Err("Dusk is already up to date.".to_string());
    }

    let assets = release
        .get("assets")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Latest Dusk release has no downloadable assets.".to_string())?;

    let asset = assets
        .iter()
        .find(|asset| {
            asset
                .get("name")
                .and_then(|value| value.as_str())
                .map(|name| {
                    let lower = name.to_ascii_lowercase();
                    lower.ends_with(".exe") && lower.contains("x64") && lower.contains("setup")
                })
                .unwrap_or(false)
        })
        .ok_or_else(|| "Latest Dusk release has no Windows x64 installer.".to_string())?;

    let download_url = asset
        .get("browser_download_url")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Dusk release installer is missing its download URL.".to_string())?;

    let expected_digest = asset
        .get("digest")
        .and_then(|value| value.as_str())
        .and_then(|value| value.strip_prefix("sha256:"))
        .map(str::to_ascii_lowercase);

    let bytes = client
        .get(download_url)
        .header(reqwest::header::USER_AGENT, "Dusk-Desktop-Updater")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("Could not download Dusk {version}: {error}"))?
        .bytes()
        .map_err(|error| format!("Could not read the Dusk installer download: {error}"))?;

    if let Some(expected) = expected_digest {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        if actual != expected {
            return Err("Downloaded Dusk installer failed SHA-256 verification.".to_string());
        }
    }

    let installer_path = env::temp_dir().join(format!("Dusk_{version}_x64-setup.exe"));
    fs::write(&installer_path, &bytes)
        .map_err(|error| format!("Could not save the Dusk installer: {error}"))?;

    Ok(installer_path)
}

#[cfg(target_os = "windows")]
fn launch_silent_update_helper(installer_path: &Path) -> Result<(), String> {
    let current_exe = env::current_exe()
        .map_err(|error| format!("Could not locate the running Dusk executable: {error}"))?;
    let current_pid = std::process::id();

    let escape_ps = |value: &str| value.replace('\'', "''");
    let installer = escape_ps(&installer_path.to_string_lossy());
    let executable = escape_ps(&current_exe.to_string_lossy());

    let script_path = env::temp_dir().join(format!("dusk-update-{}.ps1", Uuid::new_v4()));
    let script_path_ps = escape_ps(&script_path.to_string_lossy());

    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$DuskPid = {current_pid}
$Installer = '{installer}'
$DuskExe = '{executable}'
$ScriptPath = '{script_path_ps}'

try {{
    for ($i = 0; $i -lt 120; $i++) {{
        $process = Get-Process -Id $DuskPid -ErrorAction SilentlyContinue
        if ($null -eq $process) {{ break }}
        Start-Sleep -Milliseconds 250
    }}

    $process = Start-Process -FilePath $Installer -ArgumentList '/S' -PassThru -Wait
    if ($process.ExitCode -ne 0) {{
        throw "Dusk updater installer exited with code $($process.ExitCode)."
    }}

    Start-Sleep -Milliseconds 750
    if (-not (Test-Path -LiteralPath $DuskExe)) {{
        throw "Updated Dusk executable was not found after installation."
    }}

    Start-Process -FilePath $DuskExe
}} finally {{
    Remove-Item -LiteralPath $Installer -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $ScriptPath -Force -ErrorAction SilentlyContinue
}}
"#,
    );

    fs::write(&script_path, script)
        .map_err(|error| format!("Could not prepare the Dusk update helper: {error}"))?;

    let script_arg = script_path.to_string_lossy().into_owned();
    hidden_windows_command("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script_arg.as_str(),
        ])
        .spawn()
        .map_err(|error| format!("Could not start the Dusk update helper: {error}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_silent_update_helper(installer_path: &Path) -> Result<(), String> {
    Command::new(installer_path)
        .spawn()
        .map_err(|error| format!("Could not launch the Dusk installer: {error}"))?;
    Ok(())
}

#[tauri::command]
async fn install_github_update(app: AppHandle) -> Result<(), String> {
    let installer_path = tauri::async_runtime::spawn_blocking(download_latest_dusk_installer)
        .await
        .map_err(|error| format!("Dusk updater worker failed: {error}"))??;

    launch_silent_update_helper(&installer_path)?;

    // The detached helper waits for this process to exit, performs a silent
    // in-place NSIS upgrade, and then launches the updated Dusk executable.
    app.exit(0);
    Ok(())
}


fn discord_rpc_state() -> &'static Mutex<Option<DiscordIpcClient>> {
    DISCORD_RPC.get_or_init(|| Mutex::new(None))
}

fn dusk_discord_activity<'a>(state: &'a str) -> activity::Activity<'a> {
    activity::Activity::new()
        .name("Dusk")
        .activity_type(activity::ActivityType::Watching)
        .details("Dusk desktop launcher")
        .state(state)
}

#[tauri::command]
fn discord_rpc_enable(client_id: String, state: Option<String>) -> Result<(), String> {
    let client_id = client_id.trim();
    if client_id.len() < 15 || client_id.len() > 24 || !client_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err("Enter a valid Discord Application ID.".into());
    }

    let mut guard = discord_rpc_state()
        .lock()
        .map_err(|_| "Could not lock Discord Rich Presence state.".to_string())?;

    if let Some(existing) = guard.as_mut() {
        let _ = existing.clear_activity();
        let _ = existing.close();
    }

    let mut client = DiscordIpcClient::new(client_id);
    client
        .connect()
        .map_err(|error| format!("Discord is not available or RPC could not connect: {error}"))?;

    let state = state
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Browsing library");

    client
        .set_activity(dusk_discord_activity(state))
        .map_err(|error| format!("Could not publish Dusk Rich Presence: {error}"))?;

    *guard = Some(client);
    Ok(())
}

#[tauri::command]
fn discord_rpc_update(state: String) -> Result<(), String> {
    let mut guard = discord_rpc_state()
        .lock()
        .map_err(|_| "Could not lock Discord Rich Presence state.".to_string())?;

    let Some(client) = guard.as_mut() else {
        return Ok(());
    };

    let state = state.trim();
    let state = if state.is_empty() { "Browsing library" } else { state };

    if let Err(error) = client.set_activity(dusk_discord_activity(state)) {
        client
            .reconnect()
            .map_err(|reconnect_error| format!("Discord RPC reconnect failed: {reconnect_error}"))?;
        client
            .set_activity(dusk_discord_activity(state))
            .map_err(|retry_error| format!("Discord RPC update failed after reconnect: {retry_error}; first error: {error}"))?;
    }

    Ok(())
}

#[tauri::command]
fn discord_rpc_disable() -> Result<(), String> {
    let mut guard = discord_rpc_state()
        .lock()
        .map_err(|_| "Could not lock Discord Rich Presence state.".to_string())?;

    if let Some(mut client) = guard.take() {
        let _ = client.clear_activity();
        let _ = client.close();
    }

    Ok(())
}

#[tauri::command]
fn data_directory(app: AppHandle) -> Result<String, String> {
    Ok(app_data_dir(&app)?.to_string_lossy().into_owned())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Err(error) = open_database(app.handle()) {
                let diagnostic = format!(
                    "Dusk startup database initialization warning at {}\n{}\n",
                    now(),
                    error
                );
                let log_path = app
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| env::temp_dir().join("Dusk"))
                    .join("startup-error.log");
                if let Some(parent) = log_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(log_path, diagnostic);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            scan_games,
            refresh_missing_covers,
            list_profiles,
            get_active_profile,
            create_profile,
            rename_profile,
            set_active_profile,
            delete_profile,
            choose_game_installer,
            archive_import::import_game_archive,
            archive_import::import_downloaded_game_archive,
            archive_import::list_recent_game_archives,
            archive_import::verify_downloaded_multipart_archive,
            download_manager::start_managed_download,
            download_manager::start_archive_bundle,
            download_manager::list_managed_downloads,
            download_manager::cancel_managed_download,
            run_game_installer,
            open_external_target,
            search_online_fix_games,
            game_sources::search_game_source,
            game_sources::open_game_source_listing,
            game_sources::discover_game_source_archives,
            game_sources::open_game_source_search,
            game_sources::open_game_source_browser,
            get_online_fix_download_links,
            get_online_fix_hoster_files,
            open_online_fix_result,
            open_online_fix_browser,
            choose_executable,
            add_manual_game,
            set_favorite,
            rename_game,
            remove_game,
            choose_cover,
            launch_game,
            open_game_folder,
            import_screenshots,
            scan_screenshots,
            list_screenshots,
            delete_screenshot,
            list_collections,
            create_collection,
            delete_collection,
            set_collection_membership,
            collection_memberships,
            get_stats,
            list_achievements,
            save_profile_files,
            load_profile_files,
            get_profile_save_file_state,
            choose_save_folder,
            get_save_config,
            clear_save_config,
            create_save_backup,
            list_save_backups,
            restore_save_backup,
            delete_save_backup,
            upload_save_backup_to_cloud,
            upload_all_save_backups_to_cloud,
            upload_cloud_manifest,
            set_account_scope,
            export_account_state,
            export_playtime_updates,
            merge_cloud_playtime,
            import_account_state,
            windows_vpn::list_windows_vpn_profiles,
            windows_vpn::prepare_windows_vpn,
            windows_vpn::open_windows_vpn_settings,
            windows_vpn::open_cloudflare_warp_setup,
            check_github_update,
            install_github_update,
            discord_rpc_enable,
            discord_rpc_update,
            discord_rpc_disable,
            data_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dusk");
}
