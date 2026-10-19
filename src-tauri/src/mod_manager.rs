use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    time::Duration,
};
use tauri::AppHandle;
use uuid::Uuid;
use walkdir::WalkDir;

#[path = "mod_manager_thunderstore.rs"]
mod thunderstore;

const MAX_MOD_FILES: usize = 50_000;
const MAX_MOD_BYTES: u64 = 20 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModProfileRecord {
    id: String,
    name: String,
    active: bool,
    mod_count: usize,
    enabled_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModRecord {
    id: String,
    name: String,
    enabled: bool,
    file_count: usize,
    deployed_file_count: usize,
    total_bytes: u64,
    source: String,
    source_id: Option<String>,
    version: Option<String>,
    dependencies: Vec<String>,
    #[serde(skip_serializing)]
    package_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModManagerOverview {
    game_path: String,
    active_profile_id: String,
    profiles: Vec<ModProfileRecord>,
    mods: Vec<ModRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModToggleResult {
    mod_record: ModRecord,
    changed_files: usize,
    conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThunderstoreModResult {
    namespace: String,
    name: String,
    version: String,
    description: String,
    icon: Option<String>,
    downloads: u64,
    pinned: bool,
    dependency_count: usize,
    package_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThunderstoreSearchResult {
    community: String,
    game_name: String,
    loader: String,
    mods: Vec<ThunderstoreModResult>,
}

#[derive(Debug, Clone)]
struct ModFile {
    relative_path: String,
    source_path: PathBuf,
    hash: String,
}

#[derive(Debug, Clone)]
struct PlannedDeployment {
    file: ModFile,
    target: PathBuf,
    backup: Option<PathBuf>,
    original_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct Receipt {
    relative_path: String,
    deployed_hash: String,
    backup_path: Option<PathBuf>,
}

fn open_mod_database(app: &AppHandle) -> Result<Connection, String> {
    let path = super::base_app_data_dir(app)?.join("mod-manager.db");
    let connection = Connection::open(path)
        .map_err(|error| format!("Could not open Dusk mod database: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(|error| format!("Could not configure Dusk mod database: {error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(|error| format!("Could not configure Dusk mod database: {error}"))?;
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS mod_profiles (
                id TEXT PRIMARY KEY,
                game_id TEXT,
                game_path TEXT NOT NULL,
                name TEXT NOT NULL COLLATE NOCASE,
                active INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                UNIQUE(game_path, name)
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_mod_profiles_active
            ON mod_profiles(game_path) WHERE active = 1;
            CREATE TABLE IF NOT EXISTS mods (
                id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL,
                name TEXT NOT NULL,
                package_path TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                file_count INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                source TEXT NOT NULL DEFAULT 'local',
                source_id TEXT,
                version TEXT,
                dependencies_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                FOREIGN KEY(profile_id) REFERENCES mod_profiles(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_mods_profile ON mods(profile_id, created_at);
            CREATE TABLE IF NOT EXISTS mod_file_receipts (
                mod_id TEXT NOT NULL,
                game_path TEXT NOT NULL,
                relative_path TEXT NOT NULL COLLATE NOCASE,
                deployed_hash TEXT NOT NULL,
                backup_path TEXT,
                PRIMARY KEY(mod_id, relative_path),
                UNIQUE(game_path, relative_path),
                FOREIGN KEY(mod_id) REFERENCES mods(id) ON DELETE CASCADE
            );
            "#,
        )
        .map_err(|error| format!("Could not initialize Dusk mod database: {error}"))?;
    ensure_column(&connection, "mod_profiles", "game_id", "TEXT")?;
    ensure_column(
        &connection,
        "mods",
        "source",
        "TEXT NOT NULL DEFAULT 'local'",
    )?;
    ensure_column(&connection, "mods", "source_id", "TEXT")?;
    ensure_column(&connection, "mods", "version", "TEXT")?;
    ensure_column(
        &connection,
        "mods",
        "dependencies_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    connection
        .execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_mod_profiles_game_active ON mod_profiles(game_id) WHERE active = 1 AND game_id IS NOT NULL; CREATE UNIQUE INDEX IF NOT EXISTS idx_mod_source_id ON mods(profile_id, source_id) WHERE source_id IS NOT NULL;",
        )
        .map_err(|error| format!("Could not upgrade Dusk mod data: {error}"))?;
    Ok(connection)
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .any(|name| name == column);
    if !exists {
        connection
            .execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
                [],
            )
            .map_err(|error| format!("Could not upgrade Dusk mod data: {error}"))?;
    }
    Ok(())
}

fn game_root(connection: &Connection, game_id: &str) -> Result<PathBuf, String> {
    let value: String = connection
        .query_row(
            "SELECT install_path FROM games WHERE id = ?1",
            [game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not read game install folder: {error}"))?
        .ok_or_else(|| "This game is no longer in your Dusk library.".to_string())?;
    let path = PathBuf::from(value)
        .canonicalize()
        .map_err(|error| format!("Could not open the game install folder: {error}"))?;
    if !path.is_dir() || path.parent().is_none() {
        return Err(
            "The recorded game install folder is invalid. Refresh or re-add the game first.".into(),
        );
    }
    Ok(path)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn ensure_active_profile(
    connection: &Connection,
    game_id: &str,
    game_path: &str,
) -> Result<String, String> {
    if let Some(id) = connection
        .query_row(
            "SELECT id FROM mod_profiles WHERE game_id = ?1 AND active = 1",
            [game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not read active mod profile: {error}"))?
    {
        return Ok(id);
    }

    if let Some(id) = connection
        .query_row(
            "SELECT id FROM mod_profiles WHERE game_path = ?1 AND active = 1",
            [game_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not read active mod profile: {error}"))?
    {
        connection
            .execute(
                "UPDATE mod_profiles SET game_id = ?1 WHERE game_path = ?2",
                params![game_id, game_path],
            )
            .map_err(|error| format!("Could not associate mod profile with game: {error}"))?;
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    connection
        .execute(
            "INSERT INTO mod_profiles (id, game_id, game_path, name, active, created_at) VALUES (?1, ?2, ?3, 'Default', 1, ?4)",
            params![id, game_id, game_path, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("Could not create the default mod profile: {error}"))?;
    Ok(id)
}

fn bind_game_install(
    connection: &Connection,
    game_id: &str,
    game_path: &str,
) -> Result<bool, String> {
    let previous_path: Option<String> = connection
        .query_row(
            "SELECT game_path FROM mod_profiles WHERE game_id = ?1 ORDER BY active DESC, created_at LIMIT 1",
            [game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not read mod profile location: {error}"))?;
    if let Some(previous_path) = previous_path {
        if previous_path == game_path {
            return Ok(false);
        }
        if Path::new(&previous_path).exists() {
            return Err(format!(
                "Dusk found both the old and new game folders. Resolve the duplicate install before managing mods: {previous_path}"
            ));
        }
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| format!("Could not move mod profile: {error}"))?;
        transaction
            .execute(
                "UPDATE mod_profiles SET game_path = ?1 WHERE game_id = ?2",
                params![game_path, game_id],
            )
            .map_err(|error| format!("Could not move mod profile: {error}"))?;
        transaction
            .execute(
                "UPDATE mod_file_receipts SET game_path = ?1 WHERE game_path = ?2",
                params![game_path, previous_path],
            )
            .map_err(|error| format!("Could not move mod recovery records: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not finish moving mod profile: {error}"))?;
        return Ok(true);
    }

    connection
        .execute(
            "UPDATE mod_profiles SET game_id = ?1 WHERE game_path = ?2 AND game_id IS NULL",
            params![game_id, game_path],
        )
        .map_err(|error| format!("Could not associate mod profile with game: {error}"))?;
    Ok(false)
}

fn overview(
    connection: &Connection,
    root: &Path,
    game_id: &str,
) -> Result<ModManagerOverview, String> {
    let game_path = path_key(root);
    let active_profile_id = ensure_active_profile(connection, game_id, &game_path)?;
    let mut profiles_statement = connection
        .prepare(
            "SELECT p.id, p.name, p.active, COUNT(m.id), COALESCE(SUM(m.enabled), 0) FROM mod_profiles p LEFT JOIN mods m ON m.profile_id = p.id WHERE p.game_id = ?1 GROUP BY p.id ORDER BY p.created_at, p.name COLLATE NOCASE",
        )
        .map_err(|error| format!("Could not read mod profiles: {error}"))?;
    let profiles = profiles_statement
        .query_map([game_id], |row| {
            Ok(ModProfileRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                active: row.get::<_, i64>(2)? != 0,
                mod_count: row.get::<_, i64>(3)? as usize,
                enabled_count: row.get::<_, i64>(4)? as usize,
            })
        })
        .map_err(|error| format!("Could not list mod profiles: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not list mod profiles: {error}"))?;
    let mut mods_statement = connection
        .prepare(
            "SELECT m.id, m.name, m.enabled, m.file_count, m.total_bytes, m.package_path, (SELECT COUNT(*) FROM mod_file_receipts r WHERE r.mod_id = m.id), m.source, m.source_id, m.version, m.dependencies_json FROM mods m WHERE m.profile_id = ?1 ORDER BY m.created_at DESC, m.name COLLATE NOCASE",
        )
        .map_err(|error| format!("Could not read mods: {error}"))?;
    let mods = mods_statement
        .query_map([&active_profile_id], |row| {
            Ok(ModRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                file_count: row.get::<_, i64>(3)? as usize,
                total_bytes: row.get::<_, i64>(4)? as u64,
                package_path: row.get(5)?,
                deployed_file_count: row.get::<_, i64>(6)? as usize,
                source: row.get(7)?,
                source_id: row.get(8)?,
                version: row.get(9)?,
                dependencies: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
            })
        })
        .map_err(|error| format!("Could not list mods: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not list mods: {error}"))?;
    Ok(ModManagerOverview {
        game_path: root.to_string_lossy().into_owned(),
        active_profile_id,
        profiles,
        mods,
    })
}

fn validate_profile_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err("Profile names must contain 1 to 40 visible characters.".into());
    }
    Ok(name)
}

fn validate_relative_path(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err("Mod contains an unsafe file path.".into());
        };
        let part = part
            .to_str()
            .ok_or_else(|| "Mod contains a file name Dusk cannot read.".to_string())?;
        if part.is_empty()
            || part.chars().any(|character| {
                matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0')
            })
            || part.ends_with(' ')
            || part.ends_with('.')
            || part.chars().any(char::is_control)
        {
            return Err("Mod contains an unsafe file path.".into());
        }
        let device_name = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let bytes = device_name.as_bytes();
        if matches!(
            device_name.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (bytes.len() == 4
            && (bytes.starts_with(b"COM") || bytes.starts_with(b"LPT"))
            && matches!(bytes[3], b'1'..=b'9'))
        {
            return Err("Mod contains a reserved Windows file name.".into());
        }
        parts.push(part.to_string());
    }
    if parts.is_empty() {
        return Err("Mod contains an empty file path.".into());
    }
    Ok(parts.join("\\"))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("Could not read mod file {}: {error}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read mod file {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn scan_mod_files(root: &Path) -> Result<Vec<ModFile>, String> {
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut seen = HashSet::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = entry.map_err(|error| format!("Could not inspect mod folder: {error}"))?;
        if entry.file_type().is_symlink() {
            return Err("Dusk does not import mod folders containing symbolic links.".into());
        }
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            return Err("Mod folder contains a special file Dusk cannot safely install.".into());
        }
        let relative_path = validate_relative_path(
            entry
                .path()
                .strip_prefix(root)
                .map_err(|_| "Could not determine a mod file path.".to_string())?,
        )?;
        if !seen.insert(relative_path.to_ascii_lowercase()) {
            return Err("Mod contains duplicate file paths with different letter casing.".into());
        }
        let size = entry
            .metadata()
            .map_err(|error| format!("Could not inspect mod file: {error}"))?
            .len();
        total = total
            .checked_add(size)
            .ok_or_else(|| "Mod folder size overflowed.".to_string())?;
        if files.len() >= MAX_MOD_FILES || total > MAX_MOD_BYTES {
            return Err("Mod folder exceeds Dusk's limit (20 GiB / 50,000 files).".into());
        }
        files.push(ModFile {
            relative_path,
            source_path: entry.path().to_path_buf(),
            hash: hash_file(entry.path())?,
        });
    }
    if files.is_empty() {
        return Err("The selected mod folder has no files.".into());
    }
    Ok(files)
}

fn copy_mod_folder(source: &Path, destination: &Path) -> Result<(usize, u64), String> {
    let files = scan_mod_files(source)?;
    fs::create_dir_all(destination)
        .map_err(|error| format!("Could not create Dusk's mod library: {error}"))?;
    let result = (|| {
        let mut total = 0u64;
        for file in &files {
            let relative = PathBuf::from(&file.relative_path);
            let target = destination.join(relative);
            let parent = target
                .parent()
                .ok_or_else(|| "Could not create a mod file path.".to_string())?;
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create a mod folder: {error}"))?;
            total = total
                .checked_add(fs::copy(&file.source_path, &target).map_err(|error| {
                    format!("Could not copy mod file {}: {error}", file.relative_path)
                })?)
                .ok_or_else(|| "Mod folder size overflowed.".to_string())?;
        }
        Ok((files.len(), total))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

fn get_mod(connection: &Connection, mod_id: &str) -> Result<ModRecord, String> {
    connection
        .query_row(
            "SELECT m.id, m.name, m.enabled, m.file_count, m.total_bytes, m.package_path, (SELECT COUNT(*) FROM mod_file_receipts r WHERE r.mod_id = m.id), m.source, m.source_id, m.version, m.dependencies_json FROM mods m WHERE m.id = ?1",
            [mod_id],
            |row| {
                Ok(ModRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    enabled: row.get::<_, i64>(2)? != 0,
                    file_count: row.get::<_, i64>(3)? as usize,
                    total_bytes: row.get::<_, i64>(4)? as u64,
                    package_path: row.get(5)?,
                    deployed_file_count: row.get::<_, i64>(6)? as usize,
                    source: row.get(7)?,
                    source_id: row.get(8)?,
                    version: row.get(9)?,
                    dependencies: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(|error| format!("Could not read mod: {error}"))?
        .ok_or_else(|| "This mod is no longer in the active profile.".to_string())
}

fn enabled_mod_dependants(
    connection: &Connection,
    profile_id: &str,
    dependency: &ModRecord,
) -> Result<Vec<String>, String> {
    let Some(source_id) = dependency.source_id.as_deref() else {
        return Ok(Vec::new());
    };
    let mut statement = connection
        .prepare("SELECT name, dependencies_json FROM mods WHERE profile_id = ?1 AND enabled = 1 AND id != ?2")
        .map_err(|error| format!("Could not inspect mod dependencies: {error}"))?;
    let rows = statement
        .query_map(params![profile_id, dependency.id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Could not inspect mod dependencies: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not inspect mod dependencies: {error}"))?;
    let mut dependants = Vec::new();
    for (name, json) in rows {
        let dependencies: Vec<String> = serde_json::from_str(&json)
            .map_err(|error| format!("Could not read {name}'s mod dependencies: {error}"))?;
        if dependencies.iter().any(|value| value == source_id) {
            dependants.push(name);
        }
    }
    Ok(dependants)
}

fn mod_files_from_package(app_data: &Path, mod_record: &ModRecord) -> Result<Vec<ModFile>, String> {
    let library = app_data
        .join("mod-library")
        .canonicalize()
        .map_err(|error| format!("Dusk's mod library is missing: {error}"))?;
    let package = PathBuf::from(&mod_record.package_path)
        .canonicalize()
        .map_err(|error| format!("Dusk's stored mod folder is missing: {error}"))?;
    if !package.starts_with(&library) || !package.is_dir() {
        return Err("The stored mod folder is outside Dusk's managed library.".into());
    }
    scan_mod_files(&package)
}

fn check_no_symlink_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = PathBuf::from(relative);
    validate_relative_path(&relative_path)?;
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(part) = component else {
            return Err("Mod contains an unsafe file path.".into());
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("A mod target contains a symbolic link; Dusk stopped to protect files outside the game folder.".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not inspect game file {}: {error}",
                    current.display()
                ))
            }
        }
    }
    Ok(current)
}

fn deploy_mod(
    connection: &mut Connection,
    app_data: &Path,
    mod_id: &str,
    game_root: &Path,
    disable_on_failure: bool,
) -> Result<usize, String> {
    let mod_record = get_mod(connection, mod_id)?;
    let files = mod_files_from_package(app_data, &mod_record)?;
    let game_path = path_key(game_root);
    let existing_receipts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM mod_file_receipts WHERE mod_id = ?1",
            [mod_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check mod deployment: {error}"))?;
    if existing_receipts != 0 {
        return Err("Dusk still has deployed files for this mod. Disable it first to resolve the previous deployment.".into());
    }

    for file in &files {
        let occupied: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM mod_file_receipts WHERE game_path = ?1 AND relative_path = ?2)",
                params![game_path, file.relative_path],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not check mod file conflicts: {error}"))?;
        if occupied {
            return Err(format!(
                "Another enabled mod already deploys {}. Disable it before enabling this mod.",
                file.relative_path
            ));
        }
    }

    let deployment_id = Uuid::new_v4().to_string();
    let backup_root = app_data.join("mod-backups").join(&deployment_id);
    let mut planned = Vec::with_capacity(files.len());
    for file in files {
        let target = match check_no_symlink_path(game_root, &file.relative_path) {
            Ok(path) => path,
            Err(error) => {
                let _ = fs::remove_dir_all(&backup_root);
                return Err(error);
            }
        };
        let original_hash = match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    let _ = fs::remove_dir_all(&backup_root);
                    return Err(format!(
                        "A mod target is not a regular file: {}",
                        file.relative_path
                    ));
                }
                match hash_file(&target) {
                    Ok(hash) => Some(hash),
                    Err(error) => {
                        let _ = fs::remove_dir_all(&backup_root);
                        return Err(error);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                let _ = fs::remove_dir_all(&backup_root);
                return Err(format!(
                    "Could not inspect game file {}: {error}",
                    file.relative_path
                ));
            }
        };
        let backup = if original_hash.is_some() {
            let backup = backup_root.join(PathBuf::from(&file.relative_path));
            let parent = backup.parent().ok_or("Could not create mod backup path.")?;
            if let Err(error) = fs::create_dir_all(parent) {
                let _ = fs::remove_dir_all(&backup_root);
                return Err(format!("Could not create mod backup folder: {error}"));
            }
            if let Err(error) = fs::copy(&target, &backup) {
                let _ = fs::remove_dir_all(&backup_root);
                return Err(format!("Could not back up {}: {error}", file.relative_path));
            }
            Some(backup)
        } else {
            None
        };
        planned.push(PlannedDeployment {
            file,
            target,
            backup,
            original_hash,
        });
    }

    let transaction = connection
        .transaction()
        .map_err(|error| format!("Could not save mod deployment: {error}"))?;
    for item in &planned {
        let backup = item
            .backup
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        if let Err(error) = transaction.execute(
            "INSERT INTO mod_file_receipts (mod_id, game_path, relative_path, deployed_hash, backup_path) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![mod_id, game_path, item.file.relative_path, item.file.hash, backup],
        ) {
            let _ = fs::remove_dir_all(&backup_root);
            return Err(format!("Could not save deployed mod file: {error}"));
        }
    }
    if let Err(error) = transaction.execute("UPDATE mods SET enabled = 1 WHERE id = ?1", [mod_id]) {
        let _ = fs::remove_dir_all(&backup_root);
        return Err(format!("Could not save mod status: {error}"));
    }
    if let Err(error) = transaction.commit() {
        let _ = fs::remove_dir_all(&backup_root);
        return Err(format!("Could not finish mod deployment: {error}"));
    }

    for item in &planned {
        if let Err(error) = check_no_symlink_path(game_root, &item.file.relative_path) {
            let _ = undeploy_mod(connection, app_data, mod_id, game_root, disable_on_failure);
            return Err(error);
        }
        let current_hash = match fs::symlink_metadata(&item.target) {
            Ok(metadata) if metadata.is_file() => Some(hash_file(&item.target)?),
            Ok(_) => None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                let _ = undeploy_mod(connection, app_data, mod_id, game_root, disable_on_failure);
                return Err(format!(
                    "Could not inspect {}: {error}",
                    item.file.relative_path
                ));
            }
        };
        if current_hash != item.original_hash {
            let _ = undeploy_mod(connection, app_data, mod_id, game_root, disable_on_failure);
            return Err(format!(
                "{} changed while Dusk was deploying the mod; Dusk left it untouched.",
                item.file.relative_path
            ));
        }
        let parent = item
            .target
            .parent()
            .ok_or_else(|| "Could not create a game file path.".to_string())?;
        if let Err(error) = fs::create_dir_all(parent) {
            let _ = undeploy_mod(connection, app_data, mod_id, game_root, disable_on_failure);
            return Err(format!("Could not create a game folder: {error}"));
        }
        if let Err(error) = fs::copy(&item.file.source_path, &item.target) {
            let cleanup = undeploy_mod(connection, app_data, mod_id, game_root, disable_on_failure);
            let unresolved = cleanup.map(|(_, paths)| paths).unwrap_or_default();
            return Err(if unresolved.is_empty() {
                format!("Could not deploy {}: {error}", item.file.relative_path)
            } else {
                format!(
                    "Could not deploy {}: {error}. Dusk left changed files untouched: {}",
                    item.file.relative_path,
                    unresolved.join(", ")
                )
            });
        }
    }
    Ok(planned.len())
}

fn receipts_for_mod(connection: &Connection, mod_id: &str) -> Result<Vec<Receipt>, String> {
    let mut statement = connection
        .prepare("SELECT relative_path, deployed_hash, backup_path FROM mod_file_receipts WHERE mod_id = ?1 ORDER BY relative_path")
        .map_err(|error| format!("Could not read deployed mod files: {error}"))?;
    let rows = statement
        .query_map([mod_id], |row| {
            Ok(Receipt {
                relative_path: row.get(0)?,
                deployed_hash: row.get(1)?,
                backup_path: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
            })
        })
        .map_err(|error| format!("Could not read deployed mod files: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read deployed mod files: {error}"))
}

fn validated_backup(app_data: &Path, path: &Path) -> Result<Option<PathBuf>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Could not inspect Dusk's mod backup: {error}")),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Dusk's mod backup is not a regular file.".into())
        }
        Ok(_) => {
            let backup_root = app_data
                .join("mod-backups")
                .canonicalize()
                .map_err(|error| format!("Dusk's mod backup folder is unavailable: {error}"))?;
            let backup = path
                .canonicalize()
                .map_err(|error| format!("Could not open Dusk's mod backup: {error}"))?;
            if !backup.starts_with(backup_root) {
                return Err("Dusk's mod backup is outside its managed folder.".into());
            }
            Ok(Some(backup))
        }
    }
}

fn safe_to_undeploy(app_data: &Path, root: &Path, receipt: &Receipt) -> Result<bool, String> {
    let target = check_no_symlink_path(root, &receipt.relative_path)?;
    let backup = receipt
        .backup_path
        .as_ref()
        .map(|path| validated_backup(app_data, path))
        .transpose()?
        .flatten();
    match fs::symlink_metadata(&target) {
        Ok(metadata) if !metadata.is_file() => Ok(false),
        Ok(_) => {
            let current_hash = hash_file(&target)?;
            if current_hash == receipt.deployed_hash {
                return Ok(true);
            }
            if let Some(backup) = &backup {
                return Ok(backup.is_file() && hash_file(backup)? == current_hash);
            }
            Ok(false)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(receipt.backup_path.is_none() || backup.is_some())
        }
        Err(error) => Err(format!(
            "Could not inspect {}: {error}",
            receipt.relative_path
        )),
    }
}

fn undeploy_mod(
    connection: &mut Connection,
    app_data: &Path,
    mod_id: &str,
    game_root: &Path,
    set_disabled: bool,
) -> Result<(usize, Vec<String>), String> {
    let receipts = receipts_for_mod(connection, mod_id)?;
    let mut safe = Vec::new();
    let mut conflicts = Vec::new();
    for receipt in receipts {
        if safe_to_undeploy(app_data, game_root, &receipt)? {
            safe.push(receipt);
        } else {
            conflicts.push(receipt.relative_path.clone());
        }
    }

    for receipt in &safe {
        let target = check_no_symlink_path(game_root, &receipt.relative_path)?;
        let target_exists = target.is_file();
        let target_is_deployed = target_exists && hash_file(&target)? == receipt.deployed_hash;
        if target_is_deployed {
            if let Some(backup) = receipt.backup_path.as_ref() {
                let backup = validated_backup(app_data, backup)?.ok_or_else(|| {
                    format!(
                        "Dusk's original backup for {} is missing.",
                        receipt.relative_path
                    )
                })?;
                let parent = target.parent().ok_or("Could not restore a game file.")?;
                fs::create_dir_all(parent).map_err(|error| {
                    format!("Could not restore {}: {error}", receipt.relative_path)
                })?;
                fs::copy(&backup, &target).map_err(|error| {
                    format!("Could not restore {}: {error}", receipt.relative_path)
                })?;
            } else {
                fs::remove_file(&target).map_err(|error| {
                    format!("Could not remove {}: {error}", receipt.relative_path)
                })?;
            }
        } else if !target_exists {
            if let Some(backup) = receipt.backup_path.as_ref() {
                let backup = validated_backup(app_data, backup)?.ok_or_else(|| {
                    format!(
                        "Dusk's original backup for {} is missing.",
                        receipt.relative_path
                    )
                })?;
                let parent = target.parent().ok_or("Could not restore a game file.")?;
                fs::create_dir_all(parent).map_err(|error| {
                    format!("Could not restore {}: {error}", receipt.relative_path)
                })?;
                fs::copy(&backup, &target).map_err(|error| {
                    format!("Could not restore {}: {error}", receipt.relative_path)
                })?;
            }
        }
    }

    let transaction = connection
        .transaction()
        .map_err(|error| format!("Could not update mod status: {error}"))?;
    for receipt in &safe {
        transaction
            .execute(
                "DELETE FROM mod_file_receipts WHERE mod_id = ?1 AND relative_path = ?2",
                params![mod_id, receipt.relative_path],
            )
            .map_err(|error| format!("Could not update mod status: {error}"))?;
    }
    let remaining: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM mod_file_receipts WHERE mod_id = ?1",
            [mod_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not update mod status: {error}"))?;
    if set_disabled && remaining == 0 {
        transaction
            .execute("UPDATE mods SET enabled = 0 WHERE id = ?1", [mod_id])
            .map_err(|error| format!("Could not update mod status: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("Could not update mod status: {error}"))?;

    for receipt in &safe {
        if let Some(backup) = &receipt.backup_path {
            if let Ok(Some(backup)) = validated_backup(app_data, backup) {
                let _ = fs::remove_file(backup);
            }
        }
    }
    Ok((safe.len(), conflicts))
}

fn profile_mod_ids(connection: &Connection, profile_id: &str) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM mods WHERE profile_id = ?1 AND enabled = 1 ORDER BY created_at, id",
        )
        .map_err(|error| format!("Could not read profile mods: {error}"))?;
    let rows = statement
        .query_map([profile_id], |row| row.get(0))
        .map_err(|error| format!("Could not read profile mods: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not read profile mods: {error}"))
}

fn profile_mod_ids_with_receipts(
    connection: &Connection,
    profile_id: &str,
) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT DISTINCT m.id FROM mods m JOIN mod_file_receipts r ON r.mod_id = m.id WHERE m.profile_id = ?1 ORDER BY m.id")
        .map_err(|error| format!("Could not inspect profile deployments: {error}"))?;
    let rows = statement
        .query_map([profile_id], |row| row.get(0))
        .map_err(|error| format!("Could not inspect profile deployments: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not inspect profile deployments: {error}"))
}

fn preflight_profile_files(
    connection: &Connection,
    app_data: &Path,
    profile_id: &str,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM mods WHERE profile_id = ?1 AND enabled = 1 ORDER BY created_at, id",
        )
        .map_err(|error| format!("Could not inspect mod profile: {error}"))?;
    let mod_ids = statement
        .query_map([profile_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not inspect mod profile: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not inspect mod profile: {error}"))?;
    let mut seen = HashSet::new();
    for mod_id in mod_ids {
        for file in mod_files_from_package(app_data, &get_mod(connection, &mod_id)?)? {
            if !seen.insert(file.relative_path.to_lowercase()) {
                return Err(format!(
                    "This profile has two enabled mods that both contain {}. Disable one before switching profiles.",
                    file.relative_path
                ));
            }
        }
    }
    Ok(())
}

fn switch_profile(
    app_data: &Path,
    connection: &mut Connection,
    game_id: &str,
    target_profile_id: &str,
    root: &Path,
) -> Result<(), String> {
    let old_profile_id: String = connection
        .query_row(
            "SELECT id FROM mod_profiles WHERE game_id = ?1 AND active = 1",
            [game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not read active mod profile: {error}"))?
        .ok_or_else(|| "No active mod profile exists.".to_string())?;
    if old_profile_id == target_profile_id {
        return Ok(());
    }
    let belongs: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM mod_profiles WHERE id = ?1 AND game_id = ?2)",
            params![target_profile_id, game_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check mod profile: {error}"))?;
    if !belongs {
        return Err("That mod profile does not belong to this game.".into());
    }
    preflight_profile_files(connection, app_data, target_profile_id)?;

    let old_mods = profile_mod_ids(connection, &old_profile_id)?;
    let new_mods = profile_mod_ids(connection, target_profile_id)?;
    for mod_id in &old_mods {
        let receipts = receipts_for_mod(connection, mod_id)?;
        for receipt in receipts {
            if !safe_to_undeploy(app_data, root, &receipt)? {
                return Err(format!(
                    "Dusk cannot switch profiles because {} changed after deployment. Restore it or keep the current profile active.",
                    receipt.relative_path
                ));
            }
        }
    }

    for mod_id in profile_mod_ids_with_receipts(connection, target_profile_id)? {
        for receipt in receipts_for_mod(connection, &mod_id)? {
            if !safe_to_undeploy(app_data, root, &receipt)? {
                return Err(format!(
                    "Dusk cannot switch profiles because {} from the selected profile still needs review.",
                    receipt.relative_path
                ));
            }
        }
        let (_, conflicts) = undeploy_mod(connection, app_data, &mod_id, root, false)?;
        if !conflicts.is_empty() {
            return Err(
                "Dusk stopped because files from the selected profile changed after deployment."
                    .into(),
            );
        }
    }

    let mut removed = Vec::new();
    for mod_id in &old_mods {
        match undeploy_mod(connection, app_data, mod_id, root, false) {
            Ok((_, conflicts)) if conflicts.is_empty() => removed.push(mod_id.clone()),
            Ok(_) => {
                for previous in removed.iter().rev() {
                    let _ = deploy_mod(connection, app_data, previous, root, false);
                }
                return Err("Dusk stopped the profile switch because a game file changed during the operation.".into());
            }
            Err(error) => {
                for previous in removed.iter().rev() {
                    let _ = deploy_mod(connection, app_data, previous, root, false);
                }
                return Err(error);
            }
        }
    }

    let activate = (|| {
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE mod_profiles SET active = 0 WHERE game_id = ?1",
                [game_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE mod_profiles SET active = 1 WHERE id = ?1 AND game_id = ?2",
                params![target_profile_id, game_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    })();
    if let Err(error) = activate {
        for previous in removed.iter().rev() {
            let _ = deploy_mod(connection, app_data, previous, root, false);
        }
        return Err(format!("Could not activate mod profile: {error}"));
    }

    for mod_id in new_mods {
        if let Err(error) = deploy_mod(connection, app_data, &mod_id, root, false) {
            let mut recovery_issues = Vec::new();
            match profile_mod_ids_with_receipts(connection, target_profile_id) {
                Ok(mods) => {
                    for active_mod in mods.iter().rev() {
                        match undeploy_mod(connection, app_data, active_mod, root, false) {
                            Ok((_, conflicts)) if conflicts.is_empty() => {}
                            Ok((_, conflicts)) => recovery_issues.push(conflicts.join(", ")),
                            Err(recovery_error) => recovery_issues.push(recovery_error),
                        }
                    }
                }
                Err(recovery_error) => recovery_issues.push(recovery_error),
            }
            let restore_active = (|| {
                let transaction = connection
                    .transaction()
                    .map_err(|error| error.to_string())?;
                transaction
                    .execute(
                        "UPDATE mod_profiles SET active = 0 WHERE game_id = ?1",
                        [game_id],
                    )
                    .map_err(|error| error.to_string())?;
                transaction
                    .execute(
                        "UPDATE mod_profiles SET active = 1 WHERE id = ?1 AND game_id = ?2",
                        params![old_profile_id, game_id],
                    )
                    .map_err(|error| error.to_string())?;
                transaction.commit().map_err(|error| error.to_string())
            })();
            if let Err(recovery_error) = restore_active {
                recovery_issues.push(recovery_error);
            }
            for previous in removed.iter().rev() {
                if let Err(recovery_error) = deploy_mod(connection, app_data, previous, root, false)
                {
                    recovery_issues.push(recovery_error);
                }
            }
            return Err(if recovery_issues.is_empty() {
                format!("Could not activate the selected profile: {error}")
            } else {
                format!(
                    "Could not activate the selected profile: {error}. Dusk restored the previous profile where possible; review: {}",
                    recovery_issues.join("; ")
                )
            });
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn list_mod_manager(
    app: AppHandle,
    game_id: String,
) -> Result<ModManagerOverview, String> {
    let library = super::open_database(&app)?;
    let root = game_root(&library, &game_id)?;
    let connection = open_mod_database(&app)?;
    bind_game_install(&connection, &game_id, &path_key(&root))?;
    overview(&connection, &root, &game_id)
}

#[tauri::command]
pub(crate) fn create_mod_profile(
    app: AppHandle,
    game_id: String,
    name: String,
) -> Result<ModManagerOverview, String> {
    let name = validate_profile_name(&name)?;
    let library = super::open_database(&app)?;
    let root = game_root(&library, &game_id)?;
    let game_path = path_key(&root);
    let connection = open_mod_database(&app)?;
    bind_game_install(&connection, &game_id, &game_path)?;
    ensure_active_profile(&connection, &game_id, &game_path)?;
    connection
        .execute(
            "INSERT INTO mod_profiles (id, game_id, game_path, name, active, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![Uuid::new_v4().to_string(), game_id, game_path, name, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE") {
                "A mod profile with that name already exists for this game.".to_string()
            } else {
                format!("Could not create mod profile: {error}")
            }
        })?;
    overview(&connection, &root, &game_id)
}

#[tauri::command]
pub(crate) async fn set_active_mod_profile(
    app: AppHandle,
    game_id: String,
    profile_id: String,
) -> Result<ModManagerOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let library = super::open_database(&app)?;
        let root = game_root(&library, &game_id)?;
        let game_path = path_key(&root);
        let app_data = super::base_app_data_dir(&app)?;
        let mut connection = open_mod_database(&app)?;
        bind_game_install(&connection, &game_id, &game_path)?;
        ensure_active_profile(&connection, &game_id, &game_path)?;
        switch_profile(&app_data, &mut connection, &game_id, &profile_id, &root)?;
        overview(&connection, &root, &game_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn import_mod_folder(
    app: AppHandle,
    game_id: String,
) -> Result<Option<ModManagerOverview>, String> {
    let selected = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose an extracted mod folder")
            .pick_folder()
    })
    .await
    .map_err(|error| error.to_string())?;
    let Some(source) = selected else {
        return Ok(None);
    };
    tauri::async_runtime::spawn_blocking(move || {
        let library = super::open_database(&app)?;
        let root = game_root(&library, &game_id)?;
        let game_path = path_key(&root);
        let source = source
            .canonicalize()
            .map_err(|error| format!("Could not open the selected mod folder: {error}"))?;
        if !source.is_dir() {
            return Err("The selected mod source is not a folder.".into());
        }
        if source == root {
            return Err("Choose an extracted mod folder, not the whole game folder.".into());
        }
        let name = source
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.chars().count() <= 100)
            .ok_or_else(|| "Could not read the selected mod folder name.".to_string())?;
        if name.chars().any(char::is_control) {
            return Err("Mod folder names cannot contain control characters.".into());
        }
        let app_data = super::base_app_data_dir(&app)?;
        let managed_library = app_data.join("mod-library");
        fs::create_dir_all(&managed_library)
            .map_err(|error| format!("Could not prepare Dusk's mod library: {error}"))?;
        if source.starts_with(managed_library.canonicalize().map_err(|error| error.to_string())?) {
            return Err("Choose the original extracted folder, not a folder already inside Dusk's mod library.".into());
        }
        let connection = open_mod_database(&app)?;
        bind_game_install(&connection, &game_id, &game_path)?;
        let profile_id = ensure_active_profile(&connection, &game_id, &game_path)?;
        let id = Uuid::new_v4().to_string();
        let package_path = app_data.join("mod-library").join(&id);
        let (file_count, total_bytes) = copy_mod_folder(&source, &package_path)?;
        let insert = connection.execute(
            "INSERT INTO mods (id, profile_id, name, package_path, enabled, file_count, total_bytes, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7)",
            params![id, profile_id, name, package_path.to_string_lossy(), file_count as i64, total_bytes as i64, chrono::Utc::now().to_rfc3339()],
        );
        if let Err(error) = insert {
            let _ = fs::remove_dir_all(&package_path);
            return Err(format!("Could not add mod to Dusk: {error}"));
        }
        overview(&connection, &root, &game_id).map(Some)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn set_mod_enabled(
    app: AppHandle,
    game_id: String,
    mod_id: String,
    enabled: bool,
) -> Result<ModToggleResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let library = super::open_database(&app)?;
        let root = game_root(&library, &game_id)?;
        let game_path = path_key(&root);
        let app_data = super::base_app_data_dir(&app)?;
        let mut connection = open_mod_database(&app)?;
        bind_game_install(&connection, &game_id, &game_path)?;
        ensure_active_profile(&connection, &game_id, &game_path)?;
        let mod_record = get_mod(&connection, &mod_id)?;
        let profile_id: String = connection
            .query_row(
                "SELECT profile_id FROM mods WHERE id = ?1",
                [&mod_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read mod profile: {error}"))?;
        let active_profile: String = connection
            .query_row(
                "SELECT id FROM mod_profiles WHERE game_id = ?1 AND active = 1",
                [&game_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Could not read active mod profile: {error}"))?
            .ok_or_else(|| "No active mod profile exists.".to_string())?;
        if profile_id != active_profile {
            return Err("Switch to this mod profile before changing its mods.".into());
        }
        if !enabled {
            let dependants = enabled_mod_dependants(&connection, &profile_id, &mod_record)?;
            if !dependants.is_empty() {
                return Err(format!(
                    "Disable {} first; {} depend on it.",
                    mod_record.name,
                    dependants.join(", ")
                ));
            }
        }
        let receipt_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM mod_file_receipts WHERE mod_id = ?1",
                [&mod_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not check mod deployment: {error}"))?;
        let mut changed_files = 0;
        let mut conflicts = Vec::new();
        if enabled && (!mod_record.enabled || receipt_count == 0) {
            changed_files = deploy_mod(&mut connection, &app_data, &mod_id, &root, true)?;
        } else if !enabled && (mod_record.enabled || receipt_count > 0) {
            let (changed, unresolved) =
                undeploy_mod(&mut connection, &app_data, &mod_id, &root, true)?;
            changed_files = changed;
            conflicts = unresolved;
        }
        let mod_record = get_mod(&connection, &mod_id)?;
        Ok(ModToggleResult {
            mod_record,
            changed_files,
            conflicts,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn remove_mod(
    app: AppHandle,
    game_id: String,
    mod_id: String,
) -> Result<ModManagerOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let library = super::open_database(&app)?;
        let root = game_root(&library, &game_id)?;
        let game_path = path_key(&root);
        let app_data = super::base_app_data_dir(&app)?;
        let mut connection = open_mod_database(&app)?;
        bind_game_install(&connection, &game_id, &game_path)?;
        let mod_record = get_mod(&connection, &mod_id)?;
        let owner_game_id: Option<String> = connection
            .query_row(
                "SELECT p.game_id FROM mods m JOIN mod_profiles p ON p.id = m.profile_id WHERE m.id = ?1",
                [&mod_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read mod profile: {error}"))?;
        if owner_game_id.as_deref() != Some(game_id.as_str()) {
            return Err("This mod belongs to a different game.".into());
        }
        let profile_id: String = connection
            .query_row("SELECT profile_id FROM mods WHERE id = ?1", [&mod_id], |row| row.get(0))
            .map_err(|error| format!("Could not read mod profile: {error}"))?;
        let dependants = enabled_mod_dependants(&connection, &profile_id, &mod_record)?;
        if !dependants.is_empty() {
            return Err(format!(
                "Disable {} first; {} depend on it.",
                mod_record.name,
                dependants.join(", ")
            ));
        }
        let receipt_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM mod_file_receipts WHERE mod_id = ?1",
                [&mod_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not check mod deployment: {error}"))?;
        if mod_record.enabled || receipt_count > 0 {
            let (changed, conflicts) = undeploy_mod(&mut connection, &app_data, &mod_id, &root, true)?;
            if !conflicts.is_empty() {
                return Err(format!(
                    "Could not remove this mod safely. These files changed after deployment: {}. Restore them or disable the mod before removing it.",
                    conflicts.join(", ")
                ));
            }
            let _ = changed;
        }
        connection
            .execute("DELETE FROM mods WHERE id = ?1", [&mod_id])
            .map_err(|error| format!("Could not remove mod: {error}"))?;
        let package = PathBuf::from(mod_record.package_path);
        let managed = app_data.join("mod-library").canonicalize().ok();
        if package.canonicalize().ok().is_some_and(|path| managed.as_ref().is_some_and(|root| path.starts_with(root))) {
            let _ = fs::remove_dir_all(package);
        }
        overview(&connection, &root, &game_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn remove_mod_profile(
    app: AppHandle,
    game_id: String,
    profile_id: String,
) -> Result<ModManagerOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let library = super::open_database(&app)?;
        let root = game_root(&library, &game_id)?;
        let connection = open_mod_database(&app)?;
        bind_game_install(&connection, &game_id, &path_key(&root))?;
        let profile: Option<bool> = connection
            .query_row(
                "SELECT active FROM mod_profiles WHERE id = ?1 AND game_id = ?2",
                params![profile_id, game_id],
                |row| Ok(row.get::<_, i64>(0)? != 0),
            )
            .optional()
            .map_err(|error| format!("Could not read mod profile: {error}"))?;
        let Some(active) = profile else {
            return Err("That mod profile does not belong to this game.".into());
        };
        if active {
            return Err("Switch to another mod profile before removing this one.".into());
        }
        let mut statement = connection
            .prepare("SELECT package_path FROM mods WHERE profile_id = ?1")
            .map_err(|error| format!("Could not list profile mods: {error}"))?;
        let packages = statement
            .query_map([&profile_id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Could not list profile mods: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not list profile mods: {error}"))?;
        let active_receipts: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM mod_file_receipts r JOIN mods m ON m.id = r.mod_id WHERE m.profile_id = ?1)",
                [&profile_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not check profile deployments: {error}"))?;
        if active_receipts {
            return Err("This profile still has deployed files. Disable its mods before removing the profile.".into());
        }
        connection
            .execute("DELETE FROM mod_profiles WHERE id = ?1", [&profile_id])
            .map_err(|error| format!("Could not remove mod profile: {error}"))?;
        let app_data = super::base_app_data_dir(&app)?;
        let managed = app_data.join("mod-library").canonicalize().ok();
        for package in packages {
            let package = PathBuf::from(package);
            if package.canonicalize().ok().is_some_and(|path| managed.as_ref().is_some_and(|root| path.starts_with(root))) {
                let _ = fs::remove_dir_all(package);
            }
        }
        overview(&connection, &root, &game_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn search_thunderstore_mods(
    app: AppHandle,
    game_id: String,
    query: String,
) -> Result<ThunderstoreSearchResult, String> {
    thunderstore::search_mods(app, game_id, query).await
}

#[tauri::command]
pub(crate) async fn install_thunderstore_mod(
    app: AppHandle,
    game_id: String,
    namespace: String,
    name: String,
    version: String,
) -> Result<ModManagerOverview, String> {
    thunderstore::install_mod(app, game_id, namespace, name, version).await
}

#[tauri::command]
pub(crate) fn open_mod_folder(app: AppHandle, mod_id: String) -> Result<(), String> {
    let connection = open_mod_database(&app)?;
    let mod_record = get_mod(&connection, &mod_id)?;
    let app_data = super::base_app_data_dir(&app)?;
    let library = app_data
        .join("mod-library")
        .canonicalize()
        .map_err(|error| format!("Dusk's mod library is unavailable: {error}"))?;
    let package = PathBuf::from(mod_record.package_path)
        .canonicalize()
        .map_err(|error| format!("Dusk's mod folder is unavailable: {error}"))?;
    if !package.starts_with(library) || !package.is_dir() {
        return Err("The stored mod folder is outside Dusk's managed library.".into());
    }
    Command::new("explorer")
        .arg(package)
        .spawn()
        .map_err(|error| format!("Could not open mod folder: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("dusk-mod-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_mod(connection: &Connection, id: &str, package: &Path) {
        connection
            .execute_batch(
                "CREATE TABLE mod_profiles (id TEXT PRIMARY KEY, game_id TEXT, game_path TEXT, name TEXT, active INTEGER, created_at TEXT); CREATE TABLE mods (id TEXT PRIMARY KEY, profile_id TEXT, name TEXT, package_path TEXT, enabled INTEGER, file_count INTEGER, total_bytes INTEGER, source TEXT DEFAULT 'local', source_id TEXT, version TEXT, dependencies_json TEXT DEFAULT '[]', created_at TEXT); CREATE TABLE mod_file_receipts (mod_id TEXT, game_path TEXT, relative_path TEXT COLLATE NOCASE, deployed_hash TEXT, backup_path TEXT, PRIMARY KEY(mod_id, relative_path), UNIQUE(game_path, relative_path));",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO mod_profiles (id, game_path, name, active, created_at) VALUES ('p', 'game', 'Default', 1, 'now')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO mods (id, profile_id, name, package_path, enabled, file_count, total_bytes, created_at) VALUES (?1, 'p', 'Test', ?2, 0, 1, 5, 'now')",
                params![id, package.to_string_lossy()],
            )
            .unwrap();
    }

    #[test]
    fn mod_deploy_and_disable_restore_the_original_game_file() {
        let root = temp_dir();
        let app_data = root.join("app");
        let game = root.join("game");
        let package = app_data.join("mod-library").join("mod");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("config.ini"), b"original").unwrap();
        fs::write(package.join("config.ini"), b"modded").unwrap();
        let connection = Connection::open_in_memory().unwrap();
        test_mod(&connection, "m", &package);
        let mut connection = connection;

        assert_eq!(
            deploy_mod(&mut connection, &app_data, "m", &game, false).unwrap(),
            1
        );
        assert_eq!(fs::read(game.join("config.ini")).unwrap(), b"modded");
        assert!(undeploy_mod(&mut connection, &app_data, "m", &game, true)
            .unwrap()
            .1
            .is_empty());
        assert_eq!(fs::read(game.join("config.ini")).unwrap(), b"original");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disable_leaves_files_changed_outside_dusk_untouched() {
        let root = temp_dir();
        let app_data = root.join("app");
        let game = root.join("game");
        let package = app_data.join("mod-library").join("mod");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("config.ini"), b"original").unwrap();
        fs::write(package.join("config.ini"), b"modded").unwrap();
        let connection = Connection::open_in_memory().unwrap();
        test_mod(&connection, "m", &package);
        let mut connection = connection;

        deploy_mod(&mut connection, &app_data, "m", &game, false).unwrap();
        fs::write(game.join("config.ini"), b"changed by user").unwrap();
        let (_, conflicts) = undeploy_mod(&mut connection, &app_data, "m", &game, true).unwrap();
        assert_eq!(conflicts, ["config.ini"]);
        assert_eq!(
            fs::read(game.join("config.ini")).unwrap(),
            b"changed by user"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn switching_profiles_restores_then_deploys_each_mod_set() {
        let temp = temp_dir();
        let app_data = temp.join("app");
        let game = temp.join("game");
        let old_package = app_data.join("mod-library").join("old");
        let new_package = app_data.join("mod-library").join("new");
        fs::create_dir_all(&old_package).unwrap();
        fs::create_dir_all(&new_package).unwrap();
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("config.ini"), b"base").unwrap();
        fs::write(old_package.join("config.ini"), b"old mod").unwrap();
        fs::write(new_package.join("config.ini"), b"new mod").unwrap();
        let game = game.canonicalize().unwrap();
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE mod_profiles (id TEXT PRIMARY KEY, game_id TEXT, game_path TEXT, name TEXT, active INTEGER, created_at TEXT); CREATE TABLE mods (id TEXT PRIMARY KEY, profile_id TEXT, name TEXT, package_path TEXT, enabled INTEGER, file_count INTEGER, total_bytes INTEGER, source TEXT DEFAULT 'local', source_id TEXT, version TEXT, dependencies_json TEXT DEFAULT '[]', created_at TEXT); CREATE TABLE mod_file_receipts (mod_id TEXT, game_path TEXT, relative_path TEXT COLLATE NOCASE, deployed_hash TEXT, backup_path TEXT, PRIMARY KEY(mod_id, relative_path), UNIQUE(game_path, relative_path));",
            )
            .unwrap();
        let key = path_key(&game);
        connection
            .execute(
                "INSERT INTO mod_profiles VALUES ('old', 'game-id', ?1, 'Old set', 1, 'now')",
                [&key],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO mod_profiles VALUES ('new', 'game-id', ?1, 'New set', 0, 'now')",
                [&key],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO mods (id, profile_id, name, package_path, enabled, file_count, total_bytes, created_at) VALUES ('old-mod', 'old', 'Old mod', ?1, 0, 1, 7, 'now')",
                [old_package.to_string_lossy().as_ref()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO mods (id, profile_id, name, package_path, enabled, file_count, total_bytes, created_at) VALUES ('new-mod', 'new', 'New mod', ?1, 1, 1, 7, 'now')",
                [new_package.to_string_lossy().as_ref()],
            )
            .unwrap();
        let mut connection = connection;

        deploy_mod(&mut connection, &app_data, "old-mod", &game, false).unwrap();
        switch_profile(&app_data, &mut connection, "game-id", "new", &game).unwrap();
        assert_eq!(fs::read(game.join("config.ini")).unwrap(), b"new mod");
        switch_profile(&app_data, &mut connection, "game-id", "old", &game).unwrap();
        assert_eq!(fs::read(game.join("config.ini")).unwrap(), b"old mod");
        let active: String = connection
            .query_row(
                "SELECT id FROM mod_profiles WHERE game_id = ?1 AND active = 1",
                ["game-id"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, "old");
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn mod_paths_reject_traversal_and_drive_prefixes() {
        assert!(validate_relative_path(Path::new("../outside.dll")).is_err());
        assert!(validate_relative_path(Path::new("C:/outside.dll")).is_err());
        assert_eq!(
            validate_relative_path(Path::new("BepInEx/plugins/a.dll")).unwrap(),
            "BepInEx\\plugins\\a.dll"
        );
    }
}
