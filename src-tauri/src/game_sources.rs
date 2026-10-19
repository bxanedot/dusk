//! Search public game listing pages on additional websites.
//! Dusk does not attempt to bypass download-host authentication or ad gates.
use regex::Regex;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GameSourceResult {
    title: String,
    url: String,
    description: String,
}

fn source_host(source: &str) -> Result<&'static str, String> {
    match source {
        "game3rb" => Ok("game3rb.com"),
        "fitgirl" => Ok("fitgirl-repacks.site"),
        _ => Err("Unsupported game discovery source.".into()),
    }
}

pub(crate) fn verified_listing(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid game listing URL.".to_string())?;
    if parsed.scheme() != "https"
        || parsed.port().is_some()
        || parsed.username() != ""
        || parsed.password().is_some()
        || !matches!(
            parsed.host_str(),
            Some(
                "game3rb.com"
                    | "www.game3rb.com"
                    | "fitgirl-repacks.site"
                    | "www.fitgirl-repacks.site"
            )
        )
    {
        return Err("Only Game3rb or FitGirl game listings are allowed.".into());
    }
    let path = parsed.path().trim_matches('/');
    // A real WordPress game listing is /game-slug/, not /category/,
    // /tag/, /wp-admin/ or any unrelated link.
    if path.is_empty() || path.contains('/') || path.starts_with("wp-") {
        return Err("Only individual game listing pages are supported.".into());
    }
    Ok(parsed)
}

fn unescape_html(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&#8211;", "–")
        .replace("&#8217;", "'")
        .replace("&#038;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn parse_results(source: &str, html: &str) -> Result<Vec<GameSourceResult>, String> {
    let pattern = match source {
        "game3rb" => {
            r#"(?is)<h3\b[^>]*class="[^"]*\bentry-title\b[^"]*"[^>]*>\s*<a\b[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#
        }
        "fitgirl" => {
            r#"(?is)<h1\b[^>]*class="[^"]*\bentry-title\b[^"]*"[^>]*>\s*<a\b[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#
        }
        _ => return Err("Unsupported game discovery source.".into()),
    };
    let hits = Regex::new(pattern).map_err(|e| e.to_string())?;
    let tags = Regex::new(r"(?s)<[^>]+>").map_err(|e| e.to_string())?;
    let expected_host = source_host(source)?;
    let mut results: Vec<GameSourceResult> = Vec::new();
    for hit in hits.captures_iter(html) {
        let raw_url = unescape_html(hit.get(1).unwrap().as_str());
        let Ok(parsed) = verified_listing(&raw_url) else {
            continue;
        };
        if !matches!(parsed.host_str(), Some(host) if host == expected_host || host == format!("www.{expected_host}"))
        {
            continue;
        }
        let title = unescape_html(&tags.replace_all(hit.get(2).unwrap().as_str(), ""))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if title.is_empty() || results.iter().any(|entry| entry.url == raw_url) {
            continue;
        }
        results.push(GameSourceResult {
            title,
            url: raw_url,
            description: String::new(),
        });
        if results.len() == 30 {
            break;
        }
    }
    Ok(results)
}

#[tauri::command]
pub(crate) async fn search_game_source(
    source: String,
    query: String,
) -> Result<Vec<GameSourceResult>, String> {
    let host = source_host(&source)?;
    let query = query.trim().to_string();
    if query.chars().count() < 3 || query.chars().count() > 120 {
        return Err("Enter a game title between 3 and 120 characters.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(24))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let safe = attempt.url().scheme() == "https"
                    && attempt.url().host_str().map(|name| name == host || name == format!("www.{host}")).unwrap_or(false);
                if attempt.previous().len() >= 4 || !safe { attempt.stop() } else { attempt.follow() }
            }))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36")
            .build().map_err(|e| e.to_string())?;
        let url = format!("https://{host}/");
        let response = client.get(url)
            .query(&[("s", query.as_str())])
            .send().map_err(|e| format!("Could not reach {host}: {e}"))?
            .error_for_status().map_err(|e| format!("{host} search returned an error: {e}"))?;
        let mut response = response.take(4_000_001);
        use std::io::Read;
        let mut bytes = Vec::new();
        response.read_to_end(&mut bytes).map_err(|e| format!("Could not read search page: {e}"))?;
        if bytes.len() > 4_000_000 { return Err("The search page is too large to inspect.".into()); }
        let html = String::from_utf8_lossy(&bytes);
        let results = parse_results(&source, &html)?;
        if results.is_empty() && !html.contains("entry-title") {
            return Err("This website did not return a normal game search page. Try again or open it in your browser.".into());
        }
        Ok(results)
    }).await.map_err(|e| e.to_string())?
}

fn source_page_allowed(url: &reqwest::Url, expected_host: &str) -> bool {
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return false;
    }
    // Allow supported file hosts while retaining the original source restriction.
    // In-page click handling rejects unrelated pop-ups and advertising hosts.
    url.host_str()
        .map(|host| {
            host == expected_host
                || host == format!("www.{expected_host}")
                || matches!(
                    host,
                    "gofile.io"
                        | "pixeldrain.com"
                        | "mega.nz"
                        | "1fichier.com"
                        | "filecrypt.cc"
                        | "filecrypt.co"
                        | "rapidgator.net"
                        | "multiup.io"
                        | "qiwi.gg"
                        | "datanodes.to"
                        | "buzzheavier.com"
                        | "vikingfile.com"
                        | "filekeeper.net"
                        | "fileditchfiles.st"
                )
        })
        .unwrap_or(false)
}

fn open_game_source_page(app: tauri::AppHandle, listing: reqwest::Url) -> Result<(), String> {
    let host = listing
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_string();
    let title = if host == "fitgirl-repacks.site" {
        "Dusk — FitGirl (offline game listings)"
    } else {
        "Dusk — Game3rb listings"
    };
    tauri::WebviewWindowBuilder::new(
        &app,
        format!("game-source-{}", uuid::Uuid::new_v4().simple()),
        tauri::WebviewUrl::External(listing),
    )
    .title(title)
    .inner_size(1150.0, 800.0)
    .accept_first_mouse(true)
    // The ad blocker is intentionally always enabled, including new-window
    // browser sessions for Game3rb and offline FitGirl listings.
    .initialization_script(include_str!("online_fix_adblock.js"))
    .initialization_script(include_str!("game_source_navigation.js"))
    .initialization_script(include_str!("browser_copy_link.js"))
    .on_navigation(move |target| source_page_allowed(target, &host))
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .on_download(|_window, event| match event {
        tauri::webview::DownloadEvent::Requested { url, destination } => {
            let Some(filename) = destination.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            if !super::is_safe_game_archive_download(&url, filename) {
                return false;
            }
            let Some(home) = std::env::var_os("USERPROFILE") else {
                return false;
            };
            let folder = std::path::PathBuf::from(home).join("Downloads");
            if !folder.is_dir() {
                return false;
            }
            let mut path = folder.join(filename);
            if path.exists() {
                // Avoid clobbering existing files; preserving multipart filenames
                // is more important than guessing an incompatible part name.
                if filename.to_ascii_lowercase().contains(".part")
                    || filename.to_ascii_lowercase().ends_with(".7z.001")
                {
                    return false;
                }
                let fpath = std::path::Path::new(filename);
                let stem = fpath
                    .file_stem()
                    .and_then(|part| part.to_str())
                    .unwrap_or("game");
                let ext = fpath
                    .extension()
                    .and_then(|part| part.to_str())
                    .unwrap_or("zip");
                let Some(next) = (2..100)
                    .map(|i| folder.join(format!("{stem} ({i}).{ext}")))
                    .find(|candidate| !candidate.exists())
                else {
                    return false;
                };
                path = next;
            }
            *destination = path;
            true
        }
        _ => true,
    })
    .build()
    .map_err(|error| format!("Could not open game listing inside Dusk: {error}"))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn open_game_source_listing(
    app: tauri::AppHandle,
    url: String,
) -> Result<(), String> {
    open_game_source_page(app, verified_listing(&url)?)
}

// A native WebView search fallback for sites that reject the reqwest client.
// Search stays on the selected site's HTTPS origin with the same ad blocker.
#[tauri::command]
pub(crate) async fn open_game_source_search(
    app: tauri::AppHandle,
    source: String,
    query: String,
) -> Result<(), String> {
    let host = source_host(&source)?;
    let query = query.trim();
    if !(3..=120).contains(&query.chars().count()) {
        return Err("Enter a game title between 3 and 120 characters.".into());
    }
    let mut url = reqwest::Url::parse(&format!("https://{host}/")).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("s", query);
    open_game_source_page(app, url)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscoveredArchivePart {
    filename: String,
    mirrors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscoveredArchiveBundle {
    label: String,
    parts: Vec<DiscoveredArchivePart>,
}

// Only actual HTTPS archive files are eligible for direct one-click downloads.
// Mirrors for the SAME filename are alternatives, never extra copies to fetch.
// Sequential .part1.rar / .7z.001 volumes are grouped as required parts.
fn parse_direct_archives(html: &str, page: &reqwest::Url) -> Vec<DiscoveredArchiveBundle> {
    let Ok(anchors) = Regex::new(r#"(?is)<a\b[^>]*\bhref\s*=\s*["']([^"']+)["'][^>]*>"#) else {
        return Vec::new();
    };
    let mut groups: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<usize, DiscoveredArchivePart>,
    > = std::collections::BTreeMap::new();
    for capture in anchors.captures_iter(html) {
        let raw = unescape_html(capture.get(1).unwrap().as_str());
        let Ok(link) = page.join(raw.trim()) else {
            continue;
        };
        if !super::download_manager::valid_public_https(&link) {
            continue;
        }
        let Some(filename) = link
            .path_segments()
            .and_then(|mut segments| segments.next_back())
        else {
            continue;
        };
        if !super::download_manager::archive_name(filename) {
            continue;
        }
        let name = filename.to_ascii_lowercase();
        if ["crack", "patch", "update", "redist", "fix-only", "repair"]
            .iter()
            .any(|word| name.contains(word))
        {
            continue;
        }
        let (key, volume) =
            super::download_manager::multipart_volume(filename).unwrap_or_else(|| (name, 0));
        if volume > 120 {
            continue;
        }
        let group = groups.entry(key).or_default();
        let part = group
            .entry(volume)
            .or_insert_with(|| DiscoveredArchivePart {
                filename: filename.to_string(),
                mirrors: Vec::new(),
            });
        if part.mirrors.len() < 8 && !part.mirrors.iter().any(|url| url == link.as_str()) {
            part.mirrors.push(link.to_string());
        }
    }
    let mut bundles = Vec::new();
    for (label, volumes) in groups {
        let multipart = !volumes.contains_key(&0);
        if multipart {
            if volumes.len() < 2 || !volumes.contains_key(&1) {
                continue;
            }
            if volumes
                .keys()
                .copied()
                .enumerate()
                .any(|(index, volume)| volume != index + 1)
            {
                continue;
            }
        }
        let parts: Vec<_> = volumes.into_values().collect();
        if parts.is_empty() {
            continue;
        }
        bundles.push(DiscoveredArchiveBundle { label, parts });
    }
    bundles.sort_by(|a, b| {
        b.parts
            .len()
            .cmp(&a.parts.len())
            .then(a.label.cmp(&b.label))
    });
    bundles.truncate(20);
    bundles
}

// A filename-looking link does not prove the host serves a file. Some
// mirrors redirect to /download landing pages, 404s, or HTML ad gates.
// Request just the first bytes and validate the archive magic before marking
// a mirror eligible for an unattended background download.
fn archive_probe_response_ok(
    filename: &str,
    status: reqwest::StatusCode,
    content_type: &str,
    header: &[u8],
) -> bool {
    status.is_success()
        && !content_type.contains("text/html")
        && !content_type.contains("application/json")
        && super::download_manager::validate_archive_header(filename, header)
}

fn probe_direct_archive(client: &reqwest::blocking::Client, filename: &str, href: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(href) else {
        return false;
    };
    if !super::download_manager::valid_public_https(&url) {
        return false;
    }
    let Ok(mut response) = client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-15")
        .send()
    else {
        return false;
    };
    if !super::download_manager::valid_public_https(response.url())
        || !response.status().is_success()
    {
        return false;
    }
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut header = [0u8; 8];
    use std::io::Read;
    if response.read_exact(&mut header).is_err() {
        return false;
    }
    archive_probe_response_ok(filename, status, &content_type, &header)
}

// A bundle is only a valid one-click candidate when EVERY required volume
// can be served directly. Host landing pages and /download gate redirects are
// never treated as mirror URLs without a real archive response.
fn validate_direct_part(
    mut part: DiscoveredArchivePart,
    client: &reqwest::blocking::Client,
) -> Option<DiscoveredArchivePart> {
    let verified: Vec<_> = part
        .mirrors
        .iter()
        .take(4)
        .filter(|url| probe_direct_archive(client, &part.filename, url))
        .cloned()
        .collect();
    if verified.is_empty() {
        return None;
    }
    // Unverified URLs must not be reintroduced as "fallback mirrors".
    part.mirrors = verified;
    Some(part)
}

fn verify_direct_archive_bundles(
    bundles: Vec<DiscoveredArchiveBundle>,
    client: &reqwest::blocking::Client,
) -> Vec<DiscoveredArchiveBundle> {
    let mut verified = Vec::new();
    for mut bundle in bundles.into_iter().take(5) {
        if bundle.parts.is_empty() {
            continue;
        }
        let count = bundle.parts.len();
        let mut checked: Vec<Option<DiscoveredArchivePart>> = vec![None; count];
        let mut complete = true;
        // Fail fast on broken opening/closing volumes before checking the rest.
        for index in [0, count - 1] {
            if checked[index].is_some() {
                continue;
            }
            let part = bundle.parts[index].clone();
            if let Some(valid) = validate_direct_part(part, client) {
                checked[index] = Some(valid);
            } else {
                complete = false;
                break;
            }
        }
        if !complete {
            continue;
        }
        let pending = (0..count)
            .filter(|index| checked[*index].is_none())
            .collect::<Vec<_>>();
        // Bounded concurrency avoids a long sequential hang when an otherwise
        // complete 100-part bundle has one inaccessible middle volume.
        for indexes in pending.chunks(6) {
            let completed = std::thread::scope(|scope| {
                let workers = indexes
                    .iter()
                    .map(|index| {
                        let candidate = bundle.parts[*index].clone();
                        scope.spawn(move || validate_direct_part(candidate, client))
                    })
                    .collect::<Vec<_>>();
                workers
                    .into_iter()
                    .map(|worker| worker.join().ok().flatten())
                    .collect::<Vec<_>>()
            });
            for (&index, part) in indexes.iter().zip(completed) {
                if part.is_none() {
                    complete = false;
                }
                checked[index] = part;
            }
            if !complete {
                break;
            }
        }
        if !complete {
            continue;
        }
        bundle.parts = checked.into_iter().flatten().collect();
        if bundle.parts.len() == count {
            verified.push(bundle);
        }
    }
    verified
}

#[tauri::command]
pub(crate) async fn discover_game_source_archives(
    source: String,
    listing_url: String,
) -> Result<Vec<DiscoveredArchiveBundle>, String> {
    let host = source_host(&source)?;
    let page = verified_listing(&listing_url)?;
    if !matches!(page.host_str(), Some(name) if name == host || name == format!("www.{host}")) {
        return Err("The selected game listing belongs to a different source.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let safe = attempt.url().scheme() == "https" &&
                    attempt.url().host_str().is_some_and(|name| name == host || name == format!("www.{host}"));
                if !safe || attempt.previous().len() >= 3 { attempt.stop() } else { attempt.follow() }
            }))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36")
            .build().map_err(|e| e.to_string())?;
        let response = client.get(page.clone()).send()
            .map_err(|e| format!("Could not open listing: {e}"))?
            .error_for_status().map_err(|e| format!("Listing returned an error: {e}"))?;
        use std::io::Read;
        let mut limited = response.take(4_000_001);
        let mut body = Vec::new();
        limited.read_to_end(&mut body).map_err(|e| e.to_string())?;
        if body.len() > 4_000_000 { return Err("Listing is too large to inspect.".into()); }
        let candidates = parse_direct_archives(&String::from_utf8_lossy(&body), &page);
        if candidates.is_empty() { return Ok(Vec::new()); }
        let probe_client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(6))
            .timeout(Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 ||
                    !super::download_manager::valid_public_https(attempt.url()) {
                    attempt.stop()
                } else { attempt.follow() }
            }))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36")
            .build().map_err(|e| format!("Could not validate download mirrors: {e}"))?;
        Ok(verify_direct_archive_bundles(candidates, &probe_client))
    }).await.map_err(|e| e.to_string())?
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub(crate) fn open_game_source_browser(url: String) -> Result<(), String> {
    verified_listing(&url)?;
    super::hidden_windows_command("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn()
        .map_err(|e| format!("Could not open listing in browser: {e}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub(crate) fn open_game_source_browser(url: String) -> Result<(), String> {
    verified_listing(&url)?;
    Err("External browser fallback is currently supported on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bundle_can_never_pass_verification() {
        let client = reqwest::blocking::Client::new();
        let candidate = DiscoveredArchiveBundle {
            label: "empty".into(),
            parts: vec![],
        };
        assert!(verify_direct_archive_bundles(vec![candidate], &client).is_empty());
    }

    #[test]
    fn refuses_dead_or_html_mirror_responses_even_with_archive_filename() {
        let real_rar = b"Rar!\x1a\x07\x01\x00";
        assert!(!archive_probe_response_ok(
            "game.part01.rar",
            reqwest::StatusCode::NOT_FOUND,
            "application/octet-stream",
            real_rar
        ));
        assert!(!archive_probe_response_ok(
            "game.part01.rar",
            reqwest::StatusCode::OK,
            "text/html",
            b"<html>hi"
        ));
        assert!(!archive_probe_response_ok(
            "game.part01.rar",
            reqwest::StatusCode::OK,
            "application/octet-stream",
            b"<html>hi"
        ));
        assert!(archive_probe_response_ok(
            "game.part01.rar",
            reqwest::StatusCode::PARTIAL_CONTENT,
            "application/octet-stream",
            real_rar
        ));
    }

    #[test]
    fn groups_multipart_volumes_and_deduplicates_mirrors() {
        let page = reqwest::Url::parse("https://fitgirl-repacks.site/game/").unwrap();
        let html = r#"<a href="https://cdn1.example/Game.part1.rar">1</a>
            <a href="https://cdn2.example/Game.part1.rar">Mirror 1</a>
            <a href="https://cdn1.example/Game.part2.rar">2</a>
            <a href="https://ads.exoclick.com/Game.part3.rar">Bad</a>"#;
        let found = parse_direct_archives(html, &page);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].parts.len(), 2);
        assert_eq!(found[0].parts[0].mirrors.len(), 2);
    }

    #[test]
    fn rejects_incomplete_multipart_sets_and_nonarchives() {
        let page = reqwest::Url::parse("https://game3rb.com/test/").unwrap();
        let html = r#"<a href="https://cdn.example/game.7z.001"></a>
            <a href="https://cdn.example/game.7z.003"></a>
            <a href="https://cdn.example/file.exe"></a>"#;
        assert!(parse_direct_archives(html, &page).is_empty());
    }

    #[test]
    fn parses_game3rb_game_cards_not_sidebar_links() {
        let html = r#"<article class="post-hentry">
            <h3 class="g1-gamma entry-title">
            <a href="https://game3rb.com/stardew-valley-online/" rel="bookmark">Download Stardew Valley v1.6.15 + OnLine</a>
            </h3></article>
            <a href="https://game3rb.com/category/online/">Do not include</a>"#;
        let results = parse_results("game3rb", html).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Stardew Valley"));
    }

    #[test]
    fn parses_fitgirl_post_titles() {
        let html = r#"<article><header><h1 class="entry-title"><a href="https://fitgirl-repacks.site/stardew-valley/" rel="bookmark">Stardew Valley – v1.6.0 Build 24079</a></h1></header></article>"#;
        let results = parse_results("fitgirl", html).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Stardew Valley"));
    }

    #[test]
    fn embedded_browser_stays_on_chosen_source() {
        let host = "fitgirl-repacks.site";
        assert!(source_page_allowed(
            &reqwest::Url::parse("https://www.fitgirl-repacks.site/stardew-valley/").unwrap(),
            host
        ));
        assert!(source_page_allowed(
            &reqwest::Url::parse("https://fitgirl-repacks.site/?s=example").unwrap(),
            host
        ));
        assert!(!source_page_allowed(
            &reqwest::Url::parse("https://ads.example/gate").unwrap(),
            host
        ));
        assert!(!source_page_allowed(
            &reqwest::Url::parse("https://fitgirl-repacks.site.evil.org/").unwrap(),
            host
        ));
    }

    #[test]
    fn blocks_third_party_sites_and_unrelated_paths() {
        assert!(verified_listing("https://fitgirl-repacks.site/stardew-valley/").is_ok());
        assert!(verified_listing("https://game3rb.com/stardew-valley-online/").is_ok());
        assert!(verified_listing("https://game3rb.com.evil.example/foo/").is_err());
        assert!(verified_listing("http://game3rb.com/foo/").is_err());
        assert!(verified_listing("https://fitgirl-repacks.site/category/games/").is_err());
    }
}
