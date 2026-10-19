use super::*;
use std::process::Stdio;
use std::time::UNIX_EPOCH;

const MAX_UNPACKED_BYTES: u64 = 150 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 50_000;
const PYTHON_EXTRACT: &str = r#"
import pathlib, sys, zipfile, stat, shutil
archive, destination, kind, password = sys.argv[1:]
root = pathlib.Path(destination).resolve()
limit, max_files = 150 * 1024**3, 50000

def check(name):
    normalized = name.replace('\\', '/')
    if not normalized or normalized.startswith('/') or ':' in normalized or '\x00' in normalized:
        raise ValueError('Unsafe archive path')
    if '..' in normalized.split('/'):
        raise ValueError('Unsafe archive path')
    resolved = (root / normalized).resolve()
    if resolved != root and root not in resolved.parents:
        raise ValueError('Unsafe archive path')

def verify(entries):
    if len(entries) > max_files:
        raise ValueError('Too many files in archive')
    required = sum(max(0, size) for _, size, _ in entries)
    if required > limit:
        raise ValueError('Archive exceeds 150 GiB unpacked limit')
    free = shutil.disk_usage(root).free
    if free < required + 512 * 1024**2:
        raise OSError('Not enough disk space to safely extract this archive')
    for name, size, link in entries:
        check(name)
        if link:
            raise ValueError('Symlink entries are not allowed')

if kind == 'zip':
    with zipfile.ZipFile(archive) as z:
        entries = [(i.filename, i.file_size, stat.S_ISLNK(i.external_attr >> 16)) for i in z.infolist()]
        verify(entries)
        z.extractall(root, pwd=password.encode() if password else None)
elif kind == '7z':
    try:
        import py7zr
    except ImportError:
        raise RuntimeError('Install the optional Python package py7zr for 7z extraction')
    with py7zr.SevenZipFile(archive, mode='r', password=password or None) as z:
        entries = [(i.filename, i.uncompressed or 0, i.is_symlink if hasattr(i,'is_symlink') else False) for i in z.list()]
        verify(entries)
        z.extractall(path=root)
elif kind == 'rar':
    try:
        import rarfile
    except ImportError:
        raise RuntimeError('Install the optional Python package rarfile and an unrar backend for RAR extraction')
    with rarfile.RarFile(archive) as z:
        entries = [(i.filename, i.file_size, stat.S_ISLNK(i.mode) if hasattr(i,'mode') and i.mode else False) for i in z.infolist()]
        verify(entries)
        z.extractall(path=root, pwd=password or None)
else:
    raise RuntimeError('Unsupported archive type')
"#;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedGameArchive {
    directory: String,
    game: Option<GameRecord>,
    installers: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentGameArchive {
    path: String,
    filename: String,
    size_bytes: u64,
    modified_at_ms: u64,
}

fn archive_type(archive: &Path) -> Option<&'static str> {
    let filename = archive.file_name()?.to_str()?.to_ascii_lowercase();
    if filename.ends_with(".zip") {
        Some("zip")
    } else if filename.ends_with(".7z") || filename.ends_with(".7z.001") {
        Some("7z")
    } else if filename.ends_with(".rar") && !filename.contains(".part") {
        Some("rar")
    } else if super::download_manager::multipart_volume(&filename)
        .is_some_and(|(_, volume)| volume == 1 && filename.ends_with(".rar"))
    {
        Some("rar")
    } else {
        None
    }
}

fn safe_archive_entry(name: &str) -> bool {
    let normalized = name.replace('\\', "/");
    !normalized.is_empty()
        && !normalized.starts_with('/')
        && !normalized.contains(':')
        && !normalized.contains('\0')
        && !normalized.split('/').any(|part| part == "..")
}

const EXTRACTION_FREE_SPACE_RESERVE: u64 = 512 * 1024 * 1024;

fn ensure_extraction_space(destination: &Path, required: u64) -> Result<(), String> {
    let free = fs2::available_space(destination)
        .map_err(|error| format!("Could not check extraction disk space: {error}"))?;
    if free < required.saturating_add(EXTRACTION_FREE_SPACE_RESERVE) {
        let gib = 1024.0 * 1024.0 * 1024.0;
        return Err(format!(
            "Not enough free disk space for extraction: {:.1} GiB required plus 0.5 GiB reserve, {:.1} GiB available.",
            required as f64 / gib, free as f64 / gib,
        ));
    }
    Ok(())
}

fn verify_7zip_listing(output: &str) -> Result<u64, String> {
    let normalized = output.replace("\r\n", "\n");
    let body = normalized
        .splitn(2, "----------")
        .nth(1)
        .ok_or_else(|| "The archive contains no readable entries.".to_string())?;
    let mut count = 0usize;
    let mut total = 0u64;
    for block in body.split("\n\n") {
        let mut path = None;
        let mut size = 0u64;
        let mut link = false;
        for line in block.lines() {
            let line = line.trim_end_matches('\r');
            if let Some(value) = line.strip_prefix("Path = ") {
                path = Some(value);
            } else if let Some(value) = line.strip_prefix("Size = ") {
                size = value
                    .parse()
                    .map_err(|_| "Invalid archive size.".to_string())?;
            } else if line.starts_with("Symbolic Link = ")
                || line.starts_with("Hard Link = ")
                || line.starts_with("Reparse Point = ")
            {
                link = true;
            }
        }
        if let Some(name) = path {
            if !safe_archive_entry(name) || link {
                return Err("Archive contains an unsafe path or link.".into());
            }
            count += 1;
            total = total.checked_add(size).ok_or("Archive size overflow.")?;
            if count > MAX_ARCHIVE_FILES || total > MAX_UNPACKED_BYTES {
                return Err("Archive exceeds extraction limits (150 GiB / 50,000 entries).".into());
            }
        }
    }
    if count == 0 {
        return Err("No files could be found in the archive.".into());
    }
    Ok(total)
}

fn extractor_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = ["7zz", "7z", "7za", "7z.exe"]
        .iter()
        .map(PathBuf::from)
        .collect();
    #[cfg(target_os = "windows")]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(dir) = env::var_os(variable) {
                candidates.push(PathBuf::from(dir).join("7-Zip").join("7z.exe"));
            }
        }
    }
    candidates
}

fn try_7zip(archive: &Path, destination: &Path, password: Option<&str>) -> Result<bool, String> {
    for executable in extractor_candidates() {
        let mut listing = Command::new(&executable);
        listing.args(["l", "-slt", "-bd", "-y"]);
        if let Some(value) = password {
            listing.arg(format!("-p{value}"));
        }
        let listing = listing.arg(archive).stdin(Stdio::null()).output();
        let Ok(listing) = listing else { continue };
        if !listing.status.success() {
            continue;
        }
        let unpacked_size = verify_7zip_listing(&String::from_utf8_lossy(&listing.stdout))?;
        ensure_extraction_space(destination, unpacked_size)?;

        let mut command = Command::new(&executable);
        command
            .args(["x", "-y", "-bd", "-aoa"])
            .arg(format!("-o{}", destination.display()));
        if let Some(value) = password {
            command.arg(format!("-p{value}"));
        }
        let status = command.arg(archive).stdin(Stdio::null()).status();
        return Ok(matches!(status, Ok(status) if status.success()));
    }
    Ok(false)
}

#[cfg(target_os = "windows")]
fn try_windows_zip(archive: &Path, destination: &Path) -> bool {
    let script = r#"
Add-Type -AssemblyName System.IO.Compression
$zip = [System.IO.Compression.ZipFile]::OpenRead($env:DUSK_ARCHIVE)
try {
  $root = [IO.Path]::GetFullPath($env:DUSK_DESTINATION + [IO.Path]::DirectorySeparatorChar)
  $total = 0L
  if ($zip.Entries.Count -gt 50000) { throw 'Too many archive entries' }
  foreach ($entry in $zip.Entries) {
    $name = $entry.FullName.Replace('\','/')
    if ($name.StartsWith('/') -or $name.Contains(':') -or
        ($name.Split('/') -contains '..')) { throw 'Unsafe archive path' }
    $full = [IO.Path]::GetFullPath([IO.Path]::Combine($root, $name))
    if (!$full.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe archive path' }
    if ((($entry.ExternalAttributes -shr 16) -band 61440) -eq 40960) { throw 'Symlink entries are not allowed' }
    $total += $entry.Length
    if ($total -gt 161061273600L) { throw 'Archive exceeds 150 GiB limit' }
  }
  $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($root))
  if ($drive.AvailableFreeSpace -lt ($total + 536870912L)) {
    throw 'Not enough disk space to safely extract this archive'
  }
} finally { $zip.Dispose() }
[IO.Compression.ZipFile]::ExtractToDirectory($env:DUSK_ARCHIVE, $env:DUSK_DESTINATION)
"#;
    let status = hidden_windows_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("DUSK_ARCHIVE", archive)
        .env("DUSK_DESTINATION", destination)
        .stdin(Stdio::null())
        .status();
    matches!(status, Ok(value) if value.success())
}

#[cfg(not(target_os = "windows"))]
fn try_windows_zip(_archive: &Path, _destination: &Path) -> bool {
    false
}

fn try_python(archive: &Path, destination: &Path, kind: &str, password: Option<&str>) -> bool {
    for executable in ["python", "python3", "py"] {
        let status = Command::new(executable)
            .args(["-c", PYTHON_EXTRACT])
            .arg(archive)
            .arg(destination)
            .arg(kind)
            .arg(password.unwrap_or(""))
            .stdin(Stdio::null())
            .status();
        if matches!(status, Ok(value) if value.success()) {
            return true;
        }
    }
    false
}

fn extract_game_archive(
    app: AppHandle,
    archive: PathBuf,
    preferred_title: Option<String>,
    password: Option<String>,
) -> Result<ImportedGameArchive, String> {
    let archive = archive
        .canonicalize()
        .map_err(|error| format!("Archive not found: {error}"))?;
    if !archive.is_file() {
        return Err("The selected download is not a file.".into());
    }
    let kind = archive_type(&archive)
        .ok_or("Supported archives: ZIP, 7z, RAR and first multipart volumes.")?;
    let stem = preferred_title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            archive
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("Imported game")
        });
    let clean_name: String = stem
        .chars()
        .take(90)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean_name = if clean_name.trim().is_empty() {
        "Imported game"
    } else {
        clean_name.trim()
    };
    let root = app_data_dir(&app)?.join("managed-games");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let directory = root.join(format!("{clean_name}-{}", Uuid::new_v4().simple()));
    fs::create_dir(&directory).map_err(|error| error.to_string())?;

    let result = (|| {
        let extracted_7zip = try_7zip(&archive, &directory, password.as_deref())?;
        let extracted = if extracted_7zip {
            true
        } else {
            // Never let a partly extracted attempt mix with a fallback's output.
            fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;
            fs::create_dir(&directory).map_err(|error| error.to_string())?;
            if kind == "zip" && password.is_none() && try_windows_zip(&archive, &directory) {
                true
            } else {
                fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;
                fs::create_dir(&directory).map_err(|error| error.to_string())?;
                try_python(&archive, &directory, kind, password.as_deref())
            }
        };
        if !extracted {
            return Err("Could not extract archive. Check free disk space, archive integrity and passwords. Install 7-Zip, or Python 3 with py7zr (7z) / rarfile plus an unrar backend (RAR). Ensure all multipart volumes are downloaded.".into());
        }

        let mut files = 0usize;
        let mut total = 0u64;
        let mut candidates: Vec<PathBuf> = Vec::new();
        let mut installers: Vec<String> = Vec::new();
        for entry in WalkDir::new(&directory).into_iter() {
            let entry = entry.map_err(|error| format!("Could not inspect extraction: {error}"))?;
            if entry.file_type().is_symlink() {
                return Err("Extracted archive contains a symbolic link.".into());
            }
            if !entry.file_type().is_file() {
                continue;
            }
            files += 1;
            total = total
                .checked_add(entry.metadata().map_err(|error| error.to_string())?.len())
                .ok_or("Extracted size overflow.")?;
            if files > MAX_ARCHIVE_FILES || total > MAX_UNPACKED_BYTES {
                return Err("Extracted data exceeds safety limits.".into());
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext != "exe" && ext != "msi" {
                continue;
            }
            let basename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext == "msi"
                || ["setup", "install", "unins", "redist", "crash", "vc_redist"]
                    .iter()
                    .any(|prefix| basename.contains(prefix))
            {
                installers.push(path.to_string_lossy().into_owned());
            } else {
                candidates.push(path.to_path_buf());
            }
        }
        let game = if candidates.len() == 1 {
            let discovered = DiscoveredGame {
                id: format!("manual:{}", Uuid::new_v4()),
                title: clean_name.to_string(),
                exe_path: Some(candidates[0].to_string_lossy().into_owned()),
                install_path: directory.to_string_lossy().into_owned(),
                source: "manual".into(),
                source_id: None,
            };
            let connection = open_database(&app)?;
            upsert_discovered(&connection, &discovered)?;
            Some(get_game(&connection, &discovered.id)?)
        } else {
            None
        };
        Ok(ImportedGameArchive {
            directory: directory.to_string_lossy().into_owned(),
            game,
            installers,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&directory);
    }
    result
}

#[tauri::command]
pub(crate) async fn import_game_archive(
    app: AppHandle,
) -> Result<Option<ImportedGameArchive>, String> {
    let archive = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Game archive", &["zip", "7z", "rar", "001"])
            .pick_file()
    })
    .await
    .map_err(|error| error.to_string())?;
    let Some(archive) = archive else {
        return Ok(None);
    };
    tauri::async_runtime::spawn_blocking(move || extract_game_archive(app, archive, None, None))
        .await
        .map_err(|error| error.to_string())?
        .map(Some)
}

#[tauri::command]
pub(crate) async fn import_downloaded_game_archive(
    app: AppHandle,
    archive_path: String,
    title: String,
    password: Option<String>,
) -> Result<ImportedGameArchive, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let directory = downloads_dir()?;
        let file = PathBuf::from(archive_path)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !file.starts_with(&directory) || file == directory {
            return Err("Automatic imports can only access your Downloads folder.".into());
        }
        extract_game_archive(app, file, Some(title), password)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn downloads_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let home = env::var_os("USERPROFILE").ok_or("Windows user profile not found.")?;
    #[cfg(not(target_os = "windows"))]
    let home = env::var_os("HOME").ok_or("User home folder not found.")?;
    PathBuf::from(home)
        .join("Downloads")
        .canonicalize()
        .map_err(|error| format!("Downloads folder not available: {error}"))
}

// Sum all currently available multipart volumes. If a later volume arrives, the
// reported size changes and the frontend retries after the entire group settles.
// Unknown multipart sets must not be mistaken for single complete archives.
// Ensure detected parts are sequential, nonempty and no transfer is in flight.
fn combined_multipart_size(first: &Path, filename: &str) -> Option<u64> {
    let (set, first_volume) = super::download_manager::multipart_volume(filename)?;
    if first_volume != 1 {
        return None;
    }
    let mut volumes = std::collections::BTreeMap::new();
    let (prefix, _) = set.rsplit_once('.')?;
    for entry in fs::read_dir(first.parent()?).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.starts_with(prefix) {
            continue;
        }
        if name.ends_with(".crdownload") || name.ends_with(".dusk-part") || name.ends_with(".tmp") {
            return None;
        }
        let Some((candidate, index)) = super::download_manager::multipart_volume(&name) else {
            continue;
        };
        if candidate != set {
            continue;
        }
        if index > 120 {
            return None;
        }
        let meta = entry.metadata().ok()?;
        if !meta.is_file() || meta.len() == 0 {
            return None;
        }
        if volumes.insert(index, meta.len()).is_some() {
            // Different padded filenames for the same volume number are ambiguous.
            return None;
        }
    }
    if volumes.len() < 2 || !volumes.contains_key(&1) {
        return None;
    }
    if volumes
        .iter()
        .enumerate()
        .any(|(index, (number, _))| *number != index + 1)
    {
        return None;
    }
    volumes
        .values()
        .try_fold(0u64, |sum, size| sum.checked_add(*size))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveReadiness {
    pub(crate) ready: bool,
    pub(crate) message: String,
}

// Consecutive filenames are insufficient: a set with .001 and .002 may still
// be missing .003. Test the actual archive BEFORE telling the UI it is ready.
pub(crate) fn multipart_archive_readiness(
    archive: &Path,
    password: Option<&str>,
) -> ArchiveReadiness {
    let Some(filename) = archive.file_name().and_then(|value| value.to_str()) else {
        return ArchiveReadiness {
            ready: false,
            message: "Archive filename is unreadable.".into(),
        };
    };
    if super::download_manager::multipart_volume(filename).is_none_or(|(_, number)| number != 1) {
        return ArchiveReadiness {
            ready: false,
            message: "Select the first volume of a multipart archive.".into(),
        };
    }
    let Some(initial_size) = combined_multipart_size(archive, filename) else {
        return ArchiveReadiness {
            ready: false,
            message: "Waiting for at least two contiguous, nonempty archive volumes.".into(),
        };
    };

    let mut available_extractor = false;
    for executable in extractor_candidates() {
        let mut command = Command::new(&executable);
        command.args(["t", "-bd", "-y", "-bb0", "-mmt=2"]);
        if let Some(value) = password {
            command.arg(format!("-p{value}"));
        }
        match command
            .arg(archive)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => {
                available_extractor = true;
                if status.success() {
                    // Files can still be changing in Downloads while 7-Zip reads
                    // them. Never authorize extraction if any volume size moved.
                    if combined_multipart_size(archive, filename) != Some(initial_size) {
                        return ArchiveReadiness {
                            ready: false, message: "Archive parts changed during validation; waiting until transfers finish.".into(),
                        };
                    }
                    return ArchiveReadiness {
                        ready: true,
                        message: "All multipart volumes passed archive integrity testing.".into(),
                    };
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                available_extractor = true;
            }
        }
    }
    ArchiveReadiness {
        ready: false,
        message: if available_extractor {
            "Archive integrity test failed. Some volumes may be missing, incomplete or corrupt, or a password may be required.".into()
        } else {
            "Install 7-Zip to verify multipart downloads before automatic import.".into()
        },
    }
}

#[tauri::command]
pub(crate) async fn verify_downloaded_multipart_archive(
    archive_path: String,
    password: Option<String>,
) -> Result<ArchiveReadiness, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let downloads = downloads_dir()?;
        let file = PathBuf::from(&archive_path)
            .canonicalize()
            .map_err(|error| format!("Archive not found: {error}"))?;
        if file == downloads || !file.starts_with(&downloads) || !file.is_file() {
            return Err("Archive verification is restricted to files in Downloads.".into());
        }
        Ok(multipart_archive_readiness(&file, password.as_deref()))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn list_recent_game_archives(
    since_ms: u64,
) -> Result<Vec<RecentGameArchive>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let download_dir = downloads_dir()?;
        let mut archives = Vec::new();
        for entry in WalkDir::new(download_dir)
            .max_depth(3)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .take(3000)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            if archive_type(&path).is_none() {
                continue;
            }
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            if !metadata.is_file() || metadata.len() == 0 {
                continue;
            }
            let created = metadata.created().unwrap_or(UNIX_EPOCH);
            let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
            let freshest = created.max(modified);
            let modified_at_ms = freshest
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if modified_at_ms + 1000 < since_ms {
                continue;
            }
            if freshest.elapsed().unwrap_or_default() < Duration::from_secs(8) {
                continue;
            }
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("archive")
                .to_string();
            let lower = filename.to_ascii_lowercase();
            let size_bytes = if super::download_manager::multipart_volume(&lower)
                .is_some_and(|(_, number)| number == 1)
            {
                match combined_multipart_size(&path, &filename) {
                    Some(size) if size > 0 => size,
                    _ => continue, // Another volume is still downloading.
                }
            } else {
                metadata.len()
            };
            archives.push(RecentGameArchive {
                path: path.to_string_lossy().into_owned(),
                filename,
                size_bytes,
                modified_at_ms,
            });
        }
        archives.sort_by(|a, b| b.modified_at_ms.cmp(&a.modified_at_ms));
        archives.truncate(30);
        Ok::<_, String>(archives)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unsafe_archive_paths() {
        assert!(!safe_archive_entry("../game.exe"));
        assert!(!safe_archive_entry("games/../../bad.exe"));
        assert!(!safe_archive_entry("C:\\Windows\\file.exe"));
        assert!(!safe_archive_entry("/etc/passwd"));
        assert!(!safe_archive_entry("games:file"));
        assert!(safe_archive_entry("game/bin/launch.exe"));
    }

    #[test]
    fn extraction_rejects_impossible_free_space_requirements() {
        assert!(ensure_extraction_space(&std::env::temp_dir(), u64::MAX).is_err());
    }

    #[test]
    fn parses_7zip_listings_with_windows_newlines() {
        let listing = "Listing archive: demo.7z\r\n----------\r\nPath = Game/one.exe\r\nSize = 100\r\n\r\nPath = Game/two.dll\r\nSize = 200\r\n\r\n";
        assert_eq!(verify_7zip_listing(listing).unwrap(), 300);
        let unsafe_listing =
            "Listing archive: demo.7z\r\n----------\r\nPath = ../escape.exe\r\nSize = 1\r\n\r\n";
        assert!(verify_7zip_listing(unsafe_listing).is_err());
    }
}

#[cfg(test)]
mod first_volume_tests {
    use super::*;
    #[test]
    fn first_rar_volume_is_recognized_consistently() {
        assert_eq!(archive_type(Path::new("Example.part1.rar")), Some("rar"));
        assert_eq!(archive_type(Path::new("Example.part01.rar")), Some("rar"));
        assert_eq!(archive_type(Path::new("Example.part001.rar")), Some("rar"));
        assert_eq!(archive_type(Path::new("Example.part002.rar")), None);
        assert_eq!(archive_type(Path::new("Example.7z.001")), Some("7z"));
        assert_eq!(archive_type(Path::new("Example.7z.002")), None);
    }
}

#[cfg(test)]
mod multipart_scan_tests {
    use super::*;
    #[test]
    fn only_contiguous_multiple_volumes_are_scanned() {
        let dir = std::env::temp_dir().join(format!("dusk-parts-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let first = dir.join("test.part001.rar");
        fs::write(&first, b"Rar!\x1a\x07").unwrap();
        assert_eq!(combined_multipart_size(&first, "test.part001.rar"), None);
        fs::write(dir.join("test.part003.rar"), b"abcdef").unwrap();
        assert_eq!(combined_multipart_size(&first, "test.part001.rar"), None);
        fs::write(dir.join("test.part002.rar"), b"abcdef").unwrap();
        assert_eq!(
            combined_multipart_size(&first, "test.part001.rar"),
            Some(18)
        );
        fs::write(dir.join("test.part2.rar"), b"abcdef").unwrap();
        assert_eq!(combined_multipart_size(&first, "test.part001.rar"), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn consecutive_volume_names_do_not_mark_an_invalid_archive_ready() {
        let dir = std::env::temp_dir().join(format!("dusk-test-readiness-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let first = dir.join("broken.part01.rar");
        fs::write(&first, b"not actually a rar file").unwrap();
        fs::write(
            dir.join("broken.part02.rar"),
            b"not actually a continuation",
        )
        .unwrap();
        let readiness = multipart_archive_readiness(&first, None);
        assert!(
            !readiness.ready,
            "A numbered set without valid archive contents must never import"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn validates_real_multi_volume_seven_zip_archive_and_rejects_missing_last_part() {
        let Some(tool) = extractor_candidates()
            .into_iter()
            .find(|candidate| Command::new(candidate).arg("-h").output().is_ok())
        else {
            // Building Dusk must not require a separately installed extractor.
            return;
        };
        let folder =
            std::env::temp_dir().join(format!("dusk-multipart-integration-{}", Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let payload = folder.join("payload.bin");
        let mut state: u32 = 0xa5b4c3d2;
        let data: Vec<u8> = (0..4096)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state & 0xff) as u8
            })
            .collect();
        fs::write(&payload, data).unwrap();
        let destination = folder.join("fixture.7z");
        let created = Command::new(tool)
            .args(["a", "-bd", "-y", "-t7z", "-mx=0", "-v2048b"])
            .arg(&destination)
            .arg(&payload)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(
            created.success(),
            "Could not create 7-Zip multipart test fixture."
        );
        let first = folder.join("fixture.7z.001");
        assert!(first.is_file());
        let mut volumes: Vec<_> = fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|v| v.to_str())
                    .is_some_and(|name| name.starts_with("fixture.7z."))
            })
            .collect();
        volumes.sort();
        assert!(volumes.len() >= 2);
        assert!(
            multipart_archive_readiness(&first, None).ready,
            "A valid, complete multi-volume archive must pass validation"
        );
        fs::remove_file(volumes.last().unwrap()).unwrap();
        assert!(
            !multipart_archive_readiness(&first, None).ready,
            "A set missing its last part must never be treated as complete"
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn supports_game_installations_larger_than_twenty_gib() {
        assert!(MAX_UNPACKED_BYTES >= 80 * 1024 * 1024 * 1024);
    }
}
