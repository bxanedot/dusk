use super::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024 * 1024;
static JOBS: OnceLock<Mutex<HashMap<String, DownloadJob>>> = OnceLock::new();

fn jobs() -> &'static Mutex<HashMap<String, DownloadJob>> {
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadStatus {
    id: String,
    title: String,
    filename: String,
    file_path: String,
    status: String,
    received_bytes: u64,
    total_bytes: Option<u64>,
    error: Option<String>,
    bundle_ready: bool,
    listing_url: Option<String>,
    source: Option<String>,
}

struct DownloadJob {
    info: DownloadStatus,
    cancel: bool,
}

fn with_job(id: &str, update: impl FnOnce(&mut DownloadJob)) {
    if let Ok(mut map) = jobs().lock() {
        if let Some(job) = map.get_mut(id) {
            update(job);
        }
    }
}

pub(crate) fn valid_public_https(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || super::known_ad_network(&host)
    {
        return false;
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        match ip {
            IpAddr::V4(ip) => {
                !ip.is_private()
                    && !ip.is_loopback()
                    && !ip.is_link_local()
                    && !ip.is_multicast()
                    && !ip.is_unspecified()
                    && !ip.is_broadcast()
            }
            IpAddr::V6(ip) => {
                !ip.is_loopback()
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                    && !ip.is_unique_local()
                    && !ip.is_unicast_link_local()
            }
        }
    } else {
        !host.is_empty()
    }
}

pub(crate) fn archive_name(name: &str) -> bool {
    if name.len() > 210
        || name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        || name.ends_with('.')
        || name.ends_with(' ')
    {
        return false;
    }
    let name = name.to_ascii_lowercase();
    name.ends_with(".zip")
        || name.ends_with(".rar")
        || name.ends_with(".7z")
        || multipart_volume(&name).is_some_and(|(_, volume)| (1..=120).contains(&volume))
}

pub(crate) fn validate_archive_header(name: &str, bytes: &[u8]) -> bool {
    let lower = name.to_ascii_lowercase();
    // Every first volume has an archive signature; later volumes are raw
    // continuation bytes, not standalone RAR or 7z files.
    match multipart_volume(&lower) {
        Some((_, 1)) if lower.ends_with(".rar") => bytes.starts_with(b"Rar!\x1a\x07"),
        Some((_, 1)) if lower.contains(".7z.") => bytes.starts_with(b"7z\xbc\xaf\x27\x1c"),
        Some((_, volume)) if volume > 1 => !bytes.is_empty() && !looks_like_download_error(bytes),
        _ if lower.ends_with(".zip") => {
            bytes.starts_with(b"PK\x03\x04")
                || bytes.starts_with(b"PK\x05\x06")
                || bytes.starts_with(b"PK\x07\x08")
        }
        _ if lower.ends_with(".7z") => bytes.starts_with(b"7z\xbc\xaf\x27\x1c"),
        _ if lower.ends_with(".rar") => bytes.starts_with(b"Rar!\x1a\x07"),
        _ => false,
    }
}

fn looks_like_download_error(bytes: &[u8]) -> bool {
    let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(128)]);
    let head = sample
        .trim_start_matches(|ch: char| ch.is_ascii_whitespace())
        .to_ascii_lowercase();
    head.starts_with("<!doctype")
        || head.starts_with("<html")
        || head.starts_with("<?xml")
        || head.starts_with("{\"error\"")
        || head.starts_with("{\"message\"")
        || head.starts_with("access denied")
        || head.starts_with("not found")
}

fn choose_path(filename: &str) -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("USERPROFILE").ok_or("Windows user profile not found")?;
    let dir = std::path::PathBuf::from(home).join("Downloads");
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let initial = dir.join(filename);
    if !initial.exists()
        && !initial
            .with_extension(format!(
                "{}.dusk-part",
                initial.extension().and_then(|e| e.to_str()).unwrap_or("")
            ))
            .exists()
    {
        return Ok(initial);
    }
    let source = std::path::Path::new(filename);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Archive");
    let ext = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("zip");
    for number in 2..1000 {
        let candidate = dir.join(format!("{stem} ({number}).{ext}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("Could not allocate another filename in Downloads.".into())
}

fn update_complete(id: &str, state: &str, error: Option<String>) {
    with_job(id, |job| {
        job.info.status = state.into();
        job.info.error = error;
    });
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveBundlePart {
    filename: String,
    mirrors: Vec<String>,
}

// Download one validated archive, keeping partial files separate from completed
// files. The same transfer routine is used for single files and multipart bundles.
fn transfer_file(
    id: &str,
    url: reqwest::Url,
    filename: &str,
    target: &std::path::Path,
) -> Result<(), String> {
    let temp = target.with_extension(format!(
        "{}.dusk-part",
        target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("archive")
    ));
    let output = (|| -> Result<(), String> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            // Large game downloads may take hours. Reqwest's timeout covers
            // the WHOLE response, so disable that deadline; connect_timeout
            // still bounds the initial connection attempt.
            .timeout(None::<Duration>)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() > 8 || !valid_public_https(attempt.url()) {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36")
            .build().map_err(|e| e.to_string())?;
        let mut response = client
            .get(url)
            .send()
            .map_err(|e| format!("Cannot connect to download: {e}"))?
            .error_for_status()
            .map_err(|e| format!("File host declined download: {e}"))?;
        if !valid_public_https(response.url()) {
            return Err("Download was redirected to a blocked location.".into());
        }
        if let Some(size) = response.content_length() {
            if size > MAX_DOWNLOAD_BYTES {
                return Err("Archive exceeds 100 GiB download limit.".into());
            }
            with_job(id, |job| job.info.total_bytes = Some(size));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if content_type.contains("text/html") || content_type.contains("application/json") {
            return Err("The URL returned a web page, not an archive. Finish the host download steps and use the actual file link.".into());
        }

        let mut output = File::create(&temp).map_err(|e| e.to_string())?;
        let mut received = 0u64;
        let mut first = true;
        let mut chunk = [0u8; 64 * 1024];
        loop {
            if jobs()
                .lock()
                .map_err(|e| e.to_string())?
                .get(id)
                .map(|job| job.cancel)
                .unwrap_or(true)
            {
                return Err("Cancelled".into());
            }
            let size = response
                .read(&mut chunk)
                .map_err(|e| format!("Download interrupted: {e}"))?;
            if size == 0 {
                break;
            }
            if first {
                first = false;
                if !validate_archive_header(&filename, &chunk[..size]) {
                    return Err("The server did not send an archive. It may require login, cookies, or a separate download step.".into());
                }
            }
            received = received.saturating_add(size as u64);
            if received > MAX_DOWNLOAD_BYTES {
                return Err("Archive exceeds 100 GiB download limit.".into());
            }
            output
                .write_all(&chunk[..size])
                .map_err(|e| e.to_string())?;
            with_job(id, |job| job.info.received_bytes = received);
        }
        if received == 0 {
            return Err("The server returned an empty download.".into());
        }
        if let Some(expected) = response.content_length() {
            if received != expected {
                return Err("Incomplete download: server sent fewer bytes than expected.".into());
            }
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        fs::rename(&temp, &target).map_err(|e| format!("Could not save completed archive: {e}"))?;
        Ok(())
    })();

    if output.is_err() {
        let _ = fs::remove_file(&temp);
    }
    output
}

fn download_file(
    id: String,
    urls: Vec<reqwest::Url>,
    filename: String,
    target: std::path::PathBuf,
) {
    let mut failures = Vec::new();
    for url in urls {
        match transfer_file(&id, url, &filename, &target) {
            Ok(()) => {
                update_complete(&id, "completed", None);
                return;
            }
            Err(message) if message == "Cancelled" => {
                update_complete(&id, "cancelled", None);
                return;
            }
            Err(message) => failures.push(message),
        }
    }
    update_complete(
        &id,
        "failed",
        Some(format!(
            "All download mirrors failed: {}",
            failures.join(" | ")
        )),
    );
}

fn archive_password_for_source(source: Option<&str>) -> Option<&'static str> {
    match source {
        Some("online-fix") => Some("online-fix.me"),
        _ => None,
    }
}

fn download_bundle(
    id: String,
    parts: Vec<(String, Vec<reqwest::Url>)>,
    folder: std::path::PathBuf,
    source: Option<String>,
) {
    for (filename, mirrors) in &parts {
        with_job(&id, |job| {
            job.info.filename = filename.clone();
            job.info.received_bytes = 0;
            job.info.total_bytes = None;
        });
        let mut errors = Vec::new();
        let mut completed = false;
        for url in mirrors {
            match transfer_file(&id, url.clone(), filename, &folder.join(filename)) {
                Ok(()) => {
                    completed = true;
                    break;
                }
                Err(message) if message == "Cancelled" => {
                    update_complete(&id, "cancelled", None);
                    return;
                }
                Err(message) => errors.push(message),
            }
        }
        if !completed {
            update_complete(
                &id,
                "failed",
                Some(format!(
                    "Could not fetch {} from any mirror: {}",
                    filename,
                    errors.join(" | ")
                )),
            );
            return;
        }
    }
    if parts.len() > 1 && multipart_volume(&parts[0].0).is_some() {
        // A complete list of filenames and matching HTTP lengths cannot prove
        // that the archive is intact. Validate the assembled set with 7-Zip
        // before reporting a completed managed bundle to the import watcher.
        let first_path = folder.join(&parts[0].0);
        let readiness = super::archive_import::multipart_archive_readiness(
            &first_path,
            archive_password_for_source(source.as_deref()),
        );
        if !readiness.ready {
            update_complete(
                &id,
                "failed",
                Some(format!(
                    "Downloaded all archive parts, but integrity verification failed: {}",
                    readiness.message
                )),
            );
            return;
        }
    }
    update_complete(&id, "completed", None);
}

pub(crate) fn multipart_volume(filename: &str) -> Option<(String, usize)> {
    let lower = filename.to_ascii_lowercase();
    if let Some(prefix) = lower.strip_suffix(".rar") {
        if let Some((stem, digits)) = prefix.rsplit_once(".part") {
            if !stem.is_empty() && !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
            {
                return Some((format!("{stem}.rar"), digits.parse().ok()?));
            }
        }
    }
    if let Some((stem, digits)) = lower.rsplit_once(".7z.") {
        if !stem.is_empty() && digits.len() == 3 && digits.chars().all(|c| c.is_ascii_digit()) {
            return Some((format!("{stem}.7z"), digits.parse().ok()?));
        }
    }
    None
}

#[tauri::command]
pub(crate) fn start_archive_bundle(
    title: String,
    parts: Vec<ArchiveBundlePart>,
    listing_url: Option<String>,
    source: Option<String>,
) -> Result<DownloadStatus, String> {
    if source.as_deref().is_some_and(|value| value != "online-fix") {
        return Err("Unsupported download source.".into());
    }
    let listing_url = listing_url
        .map(|value| super::game_sources::verified_listing(&value).map(|url| url.to_string()))
        .transpose()?;
    if title.trim().is_empty() || title.chars().count() > 140 {
        return Err("Enter a valid game title.".into());
    }
    if parts.is_empty() || parts.len() > 120 {
        return Err("Select between 1 and 120 archive volumes.".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut ready = Vec::new();
    let mut group: Option<String> = None;
    for (index, part) in parts.into_iter().enumerate() {
        if !archive_name(&part.filename) || !seen.insert(part.filename.to_ascii_lowercase()) {
            return Err("Archive volume has an unsafe or duplicate filename.".into());
        }
        if part.mirrors.is_empty() || part.mirrors.len() > 8 {
            return Err("Each archive volume needs between 1 and 8 download mirrors.".into());
        }
        if parts_len_is_multipart(&part.filename) || index > 0 {
            let Some((stem, volume)) = multipart_volume(&part.filename) else {
                return Err("Cannot mix independent archives in one download.".into());
            };
            if index == 0 && volume != 1 {
                return Err("Multipart archive must begin at volume 1.".into());
            }
            if volume != index + 1 {
                return Err("Multipart archive volumes must be contiguous and sorted.".into());
            }
            if let Some(current) = &group {
                if current != &stem {
                    return Err("All volumes must be from one archive set.".into());
                }
            } else {
                group = Some(stem);
            }
        }
        let mut mirrors = Vec::new();
        for link in part.mirrors {
            let url =
                reqwest::Url::parse(&link).map_err(|_| "Invalid download URL.".to_string())?;
            if !valid_public_https(&url) {
                return Err("A download mirror is not a permitted public HTTPS URL.".into());
            }
            if !mirrors.contains(&url) {
                mirrors.push(url);
            }
        }
        ready.push((part.filename, mirrors));
    }
    if group.is_some() && ready.len() < 2 {
        return Err("The page lists only the first volume; the remaining parts must be available before automatic import.".into());
    }
    let id = Uuid::new_v4().to_string();
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("Home folder not found")?;
    let folder = std::path::PathBuf::from(home)
        .join("Downloads")
        .join("Dusk")
        .join(&id);
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let first = ready[0].0.clone();
    let info = DownloadStatus {
        id: id.clone(),
        title: title.trim().to_string(),
        filename: first.clone(),
        file_path: folder.join(&first).to_string_lossy().into_owned(),
        status: "downloading".into(),
        received_bytes: 0,
        total_bytes: None,
        error: None,
        bundle_ready: true,
        listing_url,
        source: source.clone(),
    };
    let mut map = jobs().lock().map_err(|e| e.to_string())?;
    if map
        .values()
        .filter(|job| job.info.status == "downloading")
        .count()
        >= 3
    {
        let _ = fs::remove_dir(&folder);
        return Err("Maximum three simultaneous downloads.".into());
    }
    map.insert(
        id.clone(),
        DownloadJob {
            info: info.clone(),
            cancel: false,
        },
    );
    drop(map);
    thread::spawn(move || download_bundle(id, ready, folder, source));
    Ok(info)
}

fn parts_len_is_multipart(filename: &str) -> bool {
    multipart_volume(filename).is_some()
}

#[tauri::command]
pub(crate) fn start_managed_download(
    url: String,
    filename: String,
    title: String,
    source: Option<String>,
) -> Result<DownloadStatus, String> {
    if source.as_deref().is_some_and(|value| value != "online-fix") {
        return Err("Unsupported download source.".into());
    }
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| "Invalid download URL".to_string())?;
    if !valid_public_https(&parsed) {
        return Err("Only public HTTPS download URLs are allowed.".into());
    }
    let filename = filename.trim().to_string();
    if !archive_name(&filename) {
        return Err("Enter a safe archive filename ending in .zip, .rar, or .7z (including supported multipart archives).".into());
    }
    if title.trim().is_empty() {
        return Err("Enter a game title.".into());
    }
    let target = choose_path(&filename)?;
    let id = Uuid::new_v4().to_string();
    let info = DownloadStatus {
        id: id.clone(),
        title: title.trim().chars().take(140).collect(),
        filename,
        file_path: target.to_string_lossy().into_owned(),
        status: "downloading".into(),
        received_bytes: 0,
        total_bytes: None,
        error: None,
        bundle_ready: false,
        listing_url: None,
        source,
    };
    let mut map = jobs().lock().map_err(|e| e.to_string())?;
    if map
        .values()
        .filter(|job| job.info.status == "downloading")
        .count()
        >= 3
    {
        return Err("Maximum three simultaneous downloads.".into());
    }
    map.insert(
        id.clone(),
        DownloadJob {
            info: info.clone(),
            cancel: false,
        },
    );
    drop(map);
    let worker_filename = info.filename.clone();
    thread::spawn(move || download_file(id, vec![parsed], worker_filename, target));
    Ok(info)
}

#[tauri::command]
pub(crate) fn list_managed_downloads() -> Result<Vec<DownloadStatus>, String> {
    let map = jobs().lock().map_err(|e| e.to_string())?;
    let mut items: Vec<_> = map.values().map(|job| job.info.clone()).collect();
    items.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(items)
}

#[tauri::command]
pub(crate) fn cancel_managed_download(download_id: String) -> Result<(), String> {
    let mut map = jobs().lock().map_err(|e| e.to_string())?;
    let job = map.get_mut(&download_id).ok_or("Download not found.")?;
    if job.info.status != "downloading" {
        return Err("Download is no longer active.".into());
    }
    job.cancel = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_known_archive_password_for_online_fix_only() {
        assert_eq!(
            archive_password_for_source(Some("online-fix")),
            Some("online-fix.me")
        );
        assert_eq!(archive_password_for_source(None), None);
        assert_eq!(archive_password_for_source(Some("fitgirl")), None);
    }

    #[test]
    fn allows_public_https_but_not_private_networks() {
        assert!(valid_public_https(
            &reqwest::Url::parse("https://cdn.example.org/game.zip").unwrap()
        ));
        for url in [
            "http://cdn.example.org/file.zip",
            "https://localhost/file.zip",
            "https://127.0.0.1/test.zip",
            "https://192.168.1.10/a.zip",
            "https://user:pass@cdn.example.org/a.zip",
            "https://sub.exoclick.com/a.zip",
        ] {
            assert!(
                !valid_public_https(&reqwest::Url::parse(url).unwrap()),
                "{url}"
            );
        }
    }
    #[test]
    fn recognizes_all_multipart_rar_volumes() {
        assert!(validate_archive_header(
            "game.part01.rar",
            b"Rar!\x1a\x07\x01"
        ));
        assert!(validate_archive_header(
            "game.part04.rar",
            b"RAW CONTINUATION BYTES"
        ));
        assert!(validate_archive_header(
            "game.part120.rar",
            b"RAW CONTINUATION BYTES"
        ));
        assert!(!validate_archive_header(
            "game.part01.rar",
            b"RAW CONTINUATION BYTES"
        ));
        assert!(!validate_archive_header(
            "game.part04.rar",
            b"<html>404 error</html>"
        ));
        assert!(!validate_archive_header(
            "game.part04.rar",
            b"{\"error\":404}"
        ));
        assert!(validate_archive_header(
            "game.7z.002",
            b"RAW CONTINUATION BYTES"
        ));
        assert!(!validate_archive_header(
            "game.7z.001",
            b"RAW CONTINUATION BYTES"
        ));
    }

    #[test]
    fn checks_filename_and_archive_signature() {
        assert!(archive_name("My Game.zip"));
        assert!(archive_name("Game.part1.rar"));
        assert!(!archive_name("../game.zip"));
        assert!(!archive_name("setup.exe"));
        assert!(validate_archive_header("game.zip", b"PK\x03\x04A"));
        assert!(validate_archive_header("game.rar", b"Rar!\x1a\x07"));
        assert!(!validate_archive_header("game.zip", b"<html>Access denied"));
    }
}
