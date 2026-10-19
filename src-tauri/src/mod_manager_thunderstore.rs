use super::*;
use flate2::read::GzDecoder;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use walkdir::WalkDir;
use zip::ZipArchive;

const ECOSYSTEM: &str = include_str!("../resources/ecosystem.json");
const CATALOG_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_INDEX_BYTES: usize = 2 * 1024 * 1024;
const MAX_SHARD_BYTES: usize = 32 * 1024 * 1024;
const MAX_SHARD_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_PACKAGE_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_DEPENDENCIES: usize = 64;
const MAX_SEARCH_RESULTS: usize = 40;

#[derive(Debug, Clone)]
struct GameRules {
    community: String,
    name: String,
    loader: String,
    package_index: String,
    install_rules: Vec<InstallRule>,
    exclusions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallRule {
    route: String,
    #[serde(default)]
    default_file_extensions: Vec<String>,
    #[serde(default)]
    tracking_method: String,
    #[serde(default)]
    sub_routes: Vec<InstallRule>,
    #[serde(default)]
    is_default_location: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogPackage {
    name: String,
    owner: String,
    package_url: String,
    #[serde(default)]
    is_pinned: bool,
    #[serde(default)]
    is_deprecated: bool,
    #[serde(default)]
    has_nsfw_content: bool,
    #[serde(default)]
    rating_score: i64,
    #[serde(default)]
    versions: Vec<CatalogVersion>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogVersion {
    version_number: String,
    #[serde(default)]
    description: String,
    icon: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    date_created: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VersionMetadata {
    namespace: String,
    name: String,
    version_number: String,
    #[serde(default)]
    dependencies: Vec<String>,
    download_url: String,
}

#[derive(Debug, Clone)]
struct PackageSpec {
    namespace: String,
    name: String,
    version: String,
    download_url: String,
    dependencies: Vec<String>,
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn ecosystem() -> Result<Value, String> {
    serde_json::from_str(ECOSYSTEM)
        .map_err(|error| format!("Dusk's Thunderstore game catalog is invalid: {error}"))
}

fn game_rules(app: &AppHandle, game_id: &str) -> Result<GameRules, String> {
    let game = super::super::list_games(app.clone())?
        .into_iter()
        .find(|game| game.id == game_id)
        .ok_or_else(|| "This game is no longer in your Dusk library.".to_string())?;
    let data = ecosystem()?;
    let games = data["games"]
        .as_object()
        .ok_or_else(|| "Dusk's Thunderstore game catalog is invalid.".to_string())?;
    let platform = match game.source.as_str() {
        "steam" => Some("steam"),
        "epic" => Some("epic-games-store"),
        "gog" => Some("gog"),
        _ => None,
    };
    let exe_name = game
        .exe_path
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .map(normalized)
        .unwrap_or_default();
    let title_key = normalized(&game.title);
    let mut selected: Option<(u8, &Value, &Value)> = None;
    for (label, definition) in games {
        let Some(profile) = definition["r2modman"]
            .as_array()
            .and_then(|entries| entries.first())
        else {
            continue;
        };
        if profile["packageIndex"].as_str().is_none()
            || profile["installRules"].as_array().is_none()
        {
            continue;
        }
        let id_match = platform
            .zip(game.source_id.as_deref())
            .is_some_and(|(platform, id)| {
                definition["distributions"]
                    .as_array()
                    .is_some_and(|distributions| {
                        distributions.iter().any(|distribution| {
                            distribution["platform"].as_str() == Some(platform)
                                && distribution["identifier"].as_str() == Some(id)
                        })
                    })
            });
        let title_match = [
            label.as_str(),
            definition["meta"]["displayName"]
                .as_str()
                .unwrap_or_default(),
            profile["meta"]["displayName"].as_str().unwrap_or_default(),
            profile["internalFolderName"].as_str().unwrap_or_default(),
            profile["steamFolderName"].as_str().unwrap_or_default(),
        ]
        .iter()
        .any(|candidate| normalized(candidate) == title_key);
        let exe_match = profile["exeNames"].as_array().is_some_and(|names| {
            names
                .iter()
                .any(|name| normalized(name.as_str().unwrap_or_default()) == exe_name)
        });
        let score = if id_match {
            3
        } else if title_match {
            2
        } else if exe_match {
            1
        } else {
            0
        };
        if score > 0 && selected.as_ref().is_none_or(|(old, _, _)| score > *old) {
            selected = Some((score, definition, profile));
        }
    }
    let Some((_, definition, profile)) = selected else {
        return Err("Thunderstore has no game-specific install rules for this game.".into());
    };
    let community = definition["label"]
        .as_str()
        .filter(|label| !label.is_empty())
        .ok_or_else(|| "Dusk could not identify this game's Thunderstore community.".to_string())?;
    let package_index = profile["packageIndex"]
        .as_str()
        .filter(|url| url.starts_with("https://thunderstore.io/c/"))
        .ok_or_else(|| "Dusk could not read this game's Thunderstore catalog URL.".to_string())?;
    let install_rules: Vec<InstallRule> =
        serde_json::from_value(profile["installRules"].clone())
            .map_err(|error| format!("Dusk could not read this game's install rules: {error}"))?;
    let exclusions = profile["relativeFileExclusions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|value| value.replace('/', "\\").to_ascii_lowercase())
        .collect();
    Ok(GameRules {
        community: community.to_string(),
        name: profile["meta"]["displayName"]
            .as_str()
            .or_else(|| definition["meta"]["displayName"].as_str())
            .unwrap_or(&game.title)
            .to_string(),
        loader: profile["packageLoader"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        package_index: package_index.to_string(),
        install_rules,
        exclusions,
    })
}

fn thunderstore_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(10 * 60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let host = attempt
                .url()
                .host_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if attempt.previous().len() > 8
                || !super::super::download_manager::valid_public_https(attempt.url())
                || !(host == "thunderstore.io" || host.ends_with(".thunderstore.io"))
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .user_agent("Dusk Mod Manager/1.11 (+https://github.com/bxanedot/dusk)")
        .build()
        .map_err(|error| format!("Could not prepare Thunderstore access: {error}"))
}

fn allowed_catalog_url(value: &str, cdn_only: bool) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| "Thunderstore returned an invalid catalog URL.".to_string())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let allowed = if cdn_only {
        host == "ccdn.thunderstore.io"
    } else {
        host == "thunderstore.io"
    };
    if !allowed || !super::super::download_manager::valid_public_https(&url) {
        return Err("Thunderstore returned a URL outside its secure catalog hosts.".into());
    }
    Ok(url)
}

fn read_bounded(mut response: reqwest::blocking::Response, max: usize) -> Result<Vec<u8>, String> {
    if !super::super::download_manager::valid_public_https(response.url()) {
        return Err("Thunderstore redirected the request to a blocked location.".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err("Thunderstore returned data larger than Dusk's catalog limit.".into());
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let size = response
            .read(&mut chunk)
            .map_err(|error| format!("Could not read Thunderstore response: {error}"))?;
        if size == 0 {
            break;
        }
        if bytes.len().saturating_add(size) > max {
            return Err("Thunderstore returned data larger than Dusk's catalog limit.".into());
        }
        bytes.extend_from_slice(&chunk[..size]);
    }
    Ok(bytes)
}

fn cached_get(
    client: &reqwest::blocking::Client,
    app_data: &Path,
    url: &str,
    cdn_only: bool,
    max: usize,
) -> Result<Vec<u8>, String> {
    let parsed = allowed_catalog_url(url, cdn_only)?;
    let community = parsed
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).nth(1))
        .filter(|segment| {
            segment.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        })
        .unwrap_or("catalog");
    let key = format!("{:x}", Sha256::digest(url.as_bytes()));
    let cache = app_data.join("mod-catalog-cache").join(community);
    fs::create_dir_all(&cache)
        .map_err(|error| format!("Could not prepare Thunderstore catalog cache: {error}"))?;
    let path = cache.join(format!("{key}.gz"));
    if fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < CATALOG_TTL)
    {
        let bytes = fs::read(&path)
            .map_err(|error| format!("Could not read the Thunderstore catalog cache: {error}"))?;
        if bytes.len() <= max {
            return Ok(bytes);
        }
    }
    let response = client
        .get(parsed)
        .send()
        .map_err(|error| format!("Could not reach Thunderstore: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Thunderstore declined the catalog request: {error}"))?;
    let bytes = read_bounded(response, max)?;
    let temp = path.with_extension("download-part");
    let mut output = File::create(&temp)
        .map_err(|error| format!("Could not cache Thunderstore catalog: {error}"))?;
    output
        .write_all(&bytes)
        .map_err(|error| format!("Could not cache Thunderstore catalog: {error}"))?;
    output
        .sync_all()
        .map_err(|error| format!("Could not cache Thunderstore catalog: {error}"))?;
    fs::rename(temp, path)
        .map_err(|error| format!("Could not finish Thunderstore catalog cache: {error}"))?;
    Ok(bytes)
}

fn decode_gzip_json(bytes: &[u8], max: usize) -> Result<Vec<u8>, String> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut decoded = Vec::new();
    decoder
        .take(max as u64 + 1)
        .read_to_end(&mut decoded)
        .map_err(|error| format!("Thunderstore's catalog shard is invalid: {error}"))?;
    if decoded.len() > max {
        return Err("Thunderstore's catalog shard exceeds Dusk's size limit.".into());
    }
    Ok(decoded)
}

fn catalog_packages(
    client: &reqwest::blocking::Client,
    app_data: &Path,
    package_index: &str,
) -> Result<Vec<CatalogPackage>, String> {
    let index_bytes = cached_get(client, app_data, package_index, false, MAX_INDEX_BYTES)?;
    let index: Vec<String> =
        serde_json::from_slice(&decode_gzip_json(&index_bytes, MAX_SHARD_JSON_BYTES)?)
            .map_err(|error| format!("Thunderstore returned an invalid package index: {error}"))?;
    if index.is_empty() || index.len() > 256 {
        return Err("Thunderstore returned an unsupported package index.".into());
    }
    let mut packages = Vec::new();
    for url in index {
        let bytes = cached_get(client, app_data, &url, true, MAX_SHARD_BYTES)?;
        let decoded = decode_gzip_json(&bytes, MAX_SHARD_JSON_BYTES)?;
        let mut shard: Vec<CatalogPackage> = serde_json::from_slice(&decoded)
            .map_err(|error| format!("Thunderstore returned an invalid package shard: {error}"))?;
        packages.append(&mut shard);
    }
    Ok(packages)
}

fn active_version(package: &CatalogPackage) -> Option<&CatalogVersion> {
    package
        .versions
        .iter()
        .filter(|version| version.is_active)
        .max_by(|left, right| left.date_created.cmp(&right.date_created))
}

pub(crate) async fn search_mods(
    app: AppHandle,
    game_id: String,
    query: String,
) -> Result<ThunderstoreSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let rules = game_rules(&app, &game_id)?;
        let app_data = super::super::base_app_data_dir(&app)?;
        let client = thunderstore_client()?;
        let terms = query.trim().to_lowercase();
        let packages = catalog_packages(&client, &app_data, &rules.package_index)?;
        let mut results: Vec<_> = packages
            .into_iter()
            .filter(|package| !package.is_deprecated && !package.has_nsfw_content)
            .filter_map(|package| {
                let version = active_version(&package)?;
                let searchable =
                    format!("{} {} {}", package.owner, package.name, version.description)
                        .to_lowercase();
                if !terms.is_empty() && !searchable.contains(&terms) {
                    return None;
                }
                Some((
                    package.rating_score,
                    package.is_pinned,
                    version.downloads,
                    ThunderstoreModResult {
                        namespace: package.owner.clone(),
                        name: package.name.clone(),
                        version: version.version_number.clone(),
                        description: version.description.chars().take(1000).collect(),
                        icon: version.icon.clone(),
                        downloads: version.downloads,
                        pinned: package.is_pinned,
                        dependency_count: version.dependencies.len(),
                        package_url: package.package_url,
                    },
                ))
            })
            .collect();
        results.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| right.0.cmp(&left.0))
                .then_with(|| right.2.cmp(&left.2))
        });
        results.truncate(MAX_SEARCH_RESULTS);
        Ok(ThunderstoreSearchResult {
            community: rules.community,
            game_name: rules.name,
            loader: rules.loader,
            mods: results
                .into_iter()
                .map(|(_, _, _, result)| result)
                .collect(),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn validate_segment(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 100
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
        || value == "."
        || value == ".."
    {
        return Err("Thunderstore returned an invalid package identifier.".into());
    }
    Ok(())
}

fn split_dependency(value: &str) -> Result<(String, String, String), String> {
    let mut parts = value.rsplitn(3, '-');
    let version = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let namespace = parts.next().unwrap_or_default();
    validate_segment(namespace)?;
    validate_segment(name)?;
    validate_segment(version)?;
    Ok((namespace.to_string(), name.to_string(), version.to_string()))
}

fn fetch_version(
    client: &reqwest::blocking::Client,
    namespace: &str,
    name: &str,
    version: &str,
) -> Result<VersionMetadata, String> {
    validate_segment(namespace)?;
    validate_segment(name)?;
    validate_segment(version)?;
    let url =
        format!("https://thunderstore.io/api/experimental/package/{namespace}/{name}/{version}/");
    let response = client
        .get(allowed_catalog_url(&url, false)?)
        .send()
        .map_err(|error| format!("Could not fetch Thunderstore package details: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Thunderstore package details were unavailable: {error}"))?;
    let bytes = read_bounded(response, 256 * 1024)?;
    let details: VersionMetadata = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Thunderstore returned invalid package details: {error}"))?;
    if details.namespace != namespace || details.name != name || details.version_number != version {
        return Err(
            "Thunderstore returned package details that do not match the selected version.".into(),
        );
    }
    let download = reqwest::Url::parse(&details.download_url)
        .map_err(|_| "Thunderstore returned an invalid package download URL.".to_string())?;
    if download.host_str() != Some("thunderstore.io")
        || !download.path().starts_with("/package/download/")
        || !super::super::download_manager::valid_public_https(&download)
    {
        return Err("Thunderstore returned an unsupported package download URL.".into());
    }
    Ok(details)
}

fn resolve_dependencies(
    client: &reqwest::blocking::Client,
    namespace: &str,
    name: &str,
    version: &str,
    visiting: &mut HashSet<String>,
    versions: &mut HashMap<String, String>,
    ordered: &mut Vec<PackageSpec>,
) -> Result<(), String> {
    let package_id = format!("{namespace}-{name}");
    if let Some(existing) = versions.get(&package_id) {
        if existing != version {
            return Err(format!(
                "This mod profile requires two versions of {package_id}; Dusk cannot install both."
            ));
        }
        return Ok(());
    }
    if ordered.len() >= MAX_DEPENDENCIES {
        return Err("This mod has too many dependencies for Dusk to install safely.".into());
    }
    if !visiting.insert(package_id.clone()) {
        return Err("Thunderstore package dependencies contain a cycle.".into());
    }
    let details = fetch_version(client, namespace, name, version)?;
    for dependency in &details.dependencies {
        let (dependency_namespace, dependency_name, dependency_version) =
            split_dependency(dependency)?;
        resolve_dependencies(
            client,
            &dependency_namespace,
            &dependency_name,
            &dependency_version,
            visiting,
            versions,
            ordered,
        )?;
    }
    visiting.remove(&package_id);
    versions.insert(package_id, version.to_string());
    ordered.push(PackageSpec {
        namespace: details.namespace,
        name: details.name,
        version: details.version_number,
        download_url: details.download_url,
        dependencies: details.dependencies,
    });
    Ok(())
}

fn download_package(
    client: &reqwest::blocking::Client,
    spec: &PackageSpec,
    target: &Path,
) -> Result<(), String> {
    let url = reqwest::Url::parse(&spec.download_url)
        .map_err(|_| "Thunderstore returned an invalid package download URL.".to_string())?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("Could not download {}: {error}", spec.name))?
        .error_for_status()
        .map_err(|error| format!("Thunderstore declined the {} download: {error}", spec.name))?;
    if !super::super::download_manager::valid_public_https(response.url())
        || response.url().host_str() != Some("ccdn.thunderstore.io")
    {
        return Err("Thunderstore redirected the mod download to an unsupported host.".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PACKAGE_ARCHIVE_BYTES)
    {
        return Err("The mod package exceeds Dusk's 2 GiB download limit.".into());
    }
    let mut response = response;
    let mut output =
        File::create(target).map_err(|error| format!("Could not create mod download: {error}"))?;
    let mut total = 0u64;
    let mut prefix = Vec::with_capacity(4);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(|error| format!("Mod download was interrupted: {error}"))?;
        if count == 0 {
            break;
        }
        if prefix.len() < 4 {
            let take = (4 - prefix.len()).min(count);
            prefix.extend_from_slice(&buffer[..take]);
            if prefix.len() == 4
                && prefix.as_slice() != b"PK\x03\x04"
                && prefix.as_slice() != b"PK\x05\x06"
            {
                return Err("Thunderstore returned a web page instead of a mod archive.".into());
            }
        }
        total = total.saturating_add(count as u64);
        if total > MAX_PACKAGE_ARCHIVE_BYTES {
            return Err("The mod package exceeds Dusk's 2 GiB download limit.".into());
        }
        output
            .write_all(&buffer[..count])
            .map_err(|error| format!("Could not save mod download: {error}"))?;
    }
    if total == 0 {
        return Err("Thunderstore returned an empty mod archive.".into());
    }
    if prefix.len() < 4 {
        return Err("Thunderstore returned an incomplete mod archive.".into());
    }
    output
        .sync_all()
        .map_err(|error| format!("Could not finish mod download: {error}"))?;
    Ok(())
}

fn extract_package_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let archive_file = File::open(archive_path)
        .map_err(|error| format!("Could not open downloaded mod archive: {error}"))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("Downloaded mod archive is not a valid ZIP file: {error}"))?;
    if archive.len() > MAX_MOD_FILES {
        return Err("Downloaded mod archive contains too many entries.".into());
    }
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not inspect mod archive: {error}"))?;
        if entry.is_symlink() {
            return Err("Downloaded mod archive contains a symbolic link.".into());
        }
        let path = entry
            .enclosed_name()
            .ok_or_else(|| "Downloaded mod archive contains an unsafe file path.".to_string())?;
        super::validate_relative_path(&path)?;
        if !entry.is_dir() {
            total = total
                .checked_add(entry.size())
                .ok_or_else(|| "Mod archive size overflowed.".to_string())?;
            if total > super::MAX_MOD_BYTES {
                return Err("Downloaded mod archive exceeds Dusk's 20 GiB expanded limit.".into());
            }
        }
    }
    let free = fs2::available_space(destination)
        .map_err(|error| format!("Could not check free space for the mod: {error}"))?;
    if free < total.saturating_add(512 * 1024 * 1024) {
        return Err("There is not enough free space to safely extract this mod.".into());
    }
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not read mod archive: {error}"))?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| "Downloaded mod archive contains an unsafe file path.".to_string())?
            .to_path_buf();
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| format!("Could not create mod folder: {error}"))?;
        } else {
            let parent = target
                .parent()
                .ok_or_else(|| "Could not create a mod file path.".to_string())?;
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create mod folder: {error}"))?;
            let mut output = File::create(&target)
                .map_err(|error| format!("Could not extract mod file: {error}"))?;
            let copied = std::io::copy(&mut entry, &mut output)
                .map_err(|error| format!("Could not extract mod file: {error}"))?;
            output
                .sync_all()
                .map_err(|error| format!("Could not finish extracting mod file: {error}"))?;
            if copied != entry.size() {
                return Err("Downloaded mod archive contains a truncated file.".into());
            }
        }
    }
    Ok(())
}

fn flatten_rules<'a>(rules: &'a [InstallRule], output: &mut Vec<&'a InstallRule>) {
    for rule in rules {
        output.push(rule);
        flatten_rules(&rule.sub_routes, output);
    }
}

fn is_metadata_file(relative: &str) -> bool {
    !relative.contains('\\')
        && matches!(
            relative.to_ascii_lowercase().as_str(),
            "manifest.json" | "icon.png" | "readme.md" | "changelog.md"
        )
}

fn loader_root_folder(namespace: &str, name: &str, loader: &str) -> Option<String> {
    let data = serde_json::from_str::<Value>(ECOSYSTEM).ok()?;
    let package_id = format!("{namespace}-{name}");
    data["modloaderPackages"]
        .as_array()?
        .iter()
        .find(|entry| {
            entry["packageId"]
                .as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(&package_id))
                && entry["loader"]
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(loader))
        })
        .and_then(|entry| entry["rootFolder"].as_str())
        .map(str::to_string)
}

fn map_package_path(
    relative: &str,
    rules: &GameRules,
    namespace: &str,
    name: &str,
) -> Result<String, String> {
    if is_metadata_file(relative)
        || rules
            .exclusions
            .iter()
            .any(|excluded| excluded == &relative.to_ascii_lowercase())
    {
        return Ok(String::new());
    }
    let mut flat = Vec::new();
    flatten_rules(&rules.install_rules, &mut flat);
    let parts: Vec<&str> = relative.split('\\').collect();
    let mut match_rule: Option<(&InstallRule, (usize, usize))> = None;
    for rule in &flat {
        let route_leaf = rule.route.rsplit('/').next().unwrap_or_default();
        if let Some(index) = parts
            .iter()
            .position(|part| part.eq_ignore_ascii_case(route_leaf))
        {
            let score = (rule.route.matches('/').count(), index);
            if match_rule.is_none_or(|(_, old_score)| score > old_score) {
                match_rule = Some((rule, score));
            }
        }
    }
    let mut suffix = relative.to_string();
    let rule = if let Some((rule, (_, index))) = match_rule {
        suffix = parts[index + 1..].join("\\");
        rule
    } else {
        let lower = relative.to_ascii_lowercase();
        flat.iter()
            .filter(|rule| {
                rule.default_file_extensions
                    .iter()
                    .any(|extension| lower.ends_with(&extension.to_ascii_lowercase()))
            })
            .max_by_key(|rule| {
                rule.default_file_extensions
                    .iter()
                    .filter(|extension| lower.ends_with(&extension.to_ascii_lowercase()))
                    .map(String::len)
                    .max()
                    .unwrap_or_default()
            })
            .copied()
            .or_else(|| flat.iter().copied().find(|rule| rule.is_default_location))
            .ok_or_else(|| "Dusk has no install rule for a file in this mod package.".to_string())?
    };
    if rule.tracking_method.eq_ignore_ascii_case("package-zip") {
        return super::validate_relative_path(Path::new(&format!(
            "{}\\{}-{}.ts.zip",
            rule.route.replace('/', "\\"),
            namespace,
            name
        )));
    }
    let route = rule.route.replace('/', "\\");
    let target = if rule.tracking_method.eq_ignore_ascii_case("none")
        || rule.tracking_method.eq_ignore_ascii_case("state")
    {
        if suffix.is_empty() {
            route
        } else {
            format!("{route}\\{suffix}")
        }
    } else if suffix.is_empty() {
        format!("{route}\\{namespace}-{name}")
    } else {
        format!("{route}\\{namespace}-{name}\\{suffix}")
    };
    super::validate_relative_path(Path::new(&target))
}

fn prepare_package(
    app_data: &Path,
    spec: &PackageSpec,
    rules: &GameRules,
    archive: &Path,
    id: &str,
) -> Result<(PathBuf, usize, u64), String> {
    let staging = app_data.join("mod-staging").join(id);
    let mapped = app_data.join("mod-staging").join(format!("{id}-mapped"));
    fs::create_dir_all(&staging)
        .map_err(|error| format!("Could not prepare mod staging folder: {error}"))?;
    let result = (|| {
        extract_package_zip(archive, &staging)?;
        let manifest_path = staging.join("manifest.json");
        let manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
                format!("Thunderstore mod package has no readable manifest: {error}")
            })?)
            .map_err(|error| format!("Thunderstore mod package manifest is invalid: {error}"))?;
        if manifest["name"].as_str() != Some(spec.name.as_str())
            || manifest["version_number"].as_str() != Some(spec.version.as_str())
        {
            return Err(
                "The downloaded mod package does not match its Thunderstore metadata.".into(),
            );
        }
        fs::create_dir_all(&mapped)
            .map_err(|error| format!("Could not prepare installed mod files: {error}"))?;
        let special_root = loader_root_folder(&spec.namespace, &spec.name, &rules.loader);
        let root_path = special_root.as_deref().map(PathBuf::from);
        let mut seen = HashSet::new();
        let mut package_zip_targets = HashSet::new();
        for entry in WalkDir::new(&staging).follow_links(false) {
            let entry = entry.map_err(|error| format!("Could not inspect mod package: {error}"))?;
            if entry.file_type().is_symlink() {
                return Err("Downloaded mod package contains a symbolic link.".into());
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(&staging)
                .map_err(|_| "Could not read a mod package path.".to_string())?;
            super::validate_relative_path(relative)?;
            let relative_text = relative.to_string_lossy().replace('/', "\\");
            let mapped_relative = if let Some(root_folder) = &root_path {
                let Some(stripped) = relative.strip_prefix(root_folder).ok() else {
                    continue;
                };
                if stripped.as_os_str().is_empty() {
                    continue;
                }
                let text = stripped.to_string_lossy().replace('/', "\\");
                if is_metadata_file(&text) {
                    continue;
                }
                super::validate_relative_path(stripped)?
            } else {
                map_package_path(&relative_text, rules, &spec.namespace, &spec.name)?
            };
            if mapped_relative.is_empty() {
                continue;
            }
            if matches!(
                Path::new(&mapped_relative)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str(),
                "exe" | "msi" | "bat" | "cmd" | "ps1" | "vbs" | "reg"
            ) {
                continue;
            }
            if mapped_relative.to_ascii_lowercase().ends_with(".ts.zip") {
                let target_key = mapped_relative.to_ascii_lowercase();
                if package_zip_targets.insert(target_key) {
                    let target = mapped.join(&mapped_relative);
                    fs::create_dir_all(target.parent().ok_or("Could not create mod route.")?)
                        .map_err(|error| format!("Could not create mod route: {error}"))?;
                    fs::copy(archive, target).map_err(|error| {
                        format!("Could not stage the mod package archive: {error}")
                    })?;
                }
                continue;
            }
            if !seen.insert(mapped_relative.to_ascii_lowercase()) {
                return Err(format!(
                    "The mod package maps more than one file to {}.",
                    mapped_relative
                ));
            }
            let target = mapped.join(&mapped_relative);
            fs::create_dir_all(target.parent().ok_or("Could not create mod route.")?)
                .map_err(|error| format!("Could not create mod route: {error}"))?;
            fs::copy(entry.path(), &target)
                .map_err(|error| format!("Could not stage {}: {error}", relative_text))?;
        }
        if seen.is_empty() && package_zip_targets.is_empty() {
            return Err("Dusk found no installable files in the Thunderstore package.".into());
        }
        let mod_files = super::scan_mod_files(&mapped)?;
        let total = mod_files
            .iter()
            .try_fold(0u64, |total, file| {
                total.checked_add(fs::metadata(&file.source_path).ok()?.len())
            })
            .ok_or_else(|| "Mod package size overflowed.".to_string())?;
        let final_path = app_data.join("mod-library").join(id);
        fs::create_dir_all(
            final_path
                .parent()
                .ok_or("Could not create Dusk's mod library.")?,
        )
        .map_err(|error| format!("Could not create Dusk's mod library: {error}"))?;
        fs::rename(&mapped, &final_path)
            .map_err(|error| format!("Could not store the installed mod: {error}"))?;
        Ok((final_path, mod_files.len(), total))
    })();
    let _ = fs::remove_dir_all(&staging);
    if result.is_err() {
        let _ = fs::remove_dir_all(&mapped);
        let _ = fs::remove_file(archive);
    }
    result
}

fn installed_mod(
    connection: &Connection,
    profile_id: &str,
    source_id: &str,
) -> Result<Option<(String, String, bool)>, String> {
    connection
        .query_row(
            "SELECT id, version, enabled FROM mods WHERE profile_id = ?1 AND source_id = ?2",
            params![profile_id, source_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
        )
        .optional()
        .map_err(|error| format!("Could not check installed Thunderstore mod: {error}"))
}

fn install_sync(
    app: AppHandle,
    game_id: String,
    namespace: String,
    name: String,
    version: String,
) -> Result<ModManagerOverview, String> {
    validate_segment(&namespace)?;
    validate_segment(&name)?;
    validate_segment(&version)?;
    let rules = game_rules(&app, &game_id)?;
    let client = thunderstore_client()?;
    let mut specs = Vec::new();
    resolve_dependencies(
        &client,
        &namespace,
        &name,
        &version,
        &mut HashSet::new(),
        &mut HashMap::new(),
        &mut specs,
    )?;
    let library = super::super::open_database(&app)?;
    let root = super::game_root(&library, &game_id)?;
    let game_path = super::path_key(&root);
    let app_data = super::super::base_app_data_dir(&app)?;
    let mut connection = super::open_mod_database(&app)?;
    super::bind_game_install(&connection, &game_id, &game_path)?;
    let profile_id = super::ensure_active_profile(&connection, &game_id, &game_path)?;
    let mut added = Vec::new();
    let mut enabled_existing = Vec::new();
    let result = (|| {
        for spec in &specs {
            let source_id = format!("{}-{}", spec.namespace, spec.name);
            if let Some((id, existing_version, enabled)) =
                installed_mod(&connection, &profile_id, &source_id)?
            {
                if existing_version != spec.version {
                    return Err(format!(
                        "{} {} is already installed. Remove it before installing version {}.",
                        source_id, existing_version, spec.version
                    ));
                }
                if !enabled {
                    super::deploy_mod(&mut connection, &app_data, &id, &root, true)?;
                    enabled_existing.push(id);
                }
                continue;
            }

            let id = Uuid::new_v4().to_string();
            let staging = app_data.join("mod-staging");
            fs::create_dir_all(&staging)
                .map_err(|error| format!("Could not prepare mod downloads: {error}"))?;
            let archive = staging.join(format!("{id}.zip"));
            if let Err(error) = download_package(&client, spec, &archive) {
                let _ = fs::remove_file(&archive);
                return Err(error);
            }
            let (package_path, file_count, total_bytes) =
                prepare_package(&app_data, spec, &rules, &archive, &id)?;
            let dependencies = spec
                .dependencies
                .iter()
                .filter_map(|dependency| split_dependency(dependency).ok())
                .map(|(namespace, name, _)| format!("{namespace}-{name}"))
                .collect::<Vec<_>>();
            let insert = connection.execute(
                "INSERT INTO mods (id, profile_id, name, package_path, enabled, file_count, total_bytes, source, source_id, version, dependencies_json, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, 'thunderstore', ?7, ?8, ?9, ?10)",
                params![
                    id,
                    profile_id,
                    format!("{} - {}", spec.namespace, spec.name),
                    package_path.to_string_lossy(),
                    file_count as i64,
                    total_bytes as i64,
                    source_id,
                    spec.version,
                    serde_json::to_string(&dependencies).map_err(|error| error.to_string())?,
                    chrono::Utc::now().to_rfc3339(),
                ],
            );
            if let Err(error) = insert {
                let _ = fs::remove_dir_all(&package_path);
                return Err(format!("Could not add the downloaded mod: {error}"));
            }
            if let Err(error) = super::deploy_mod(&mut connection, &app_data, &id, &root, true) {
                let _ = connection.execute("DELETE FROM mods WHERE id = ?1", [&id]);
                let _ = fs::remove_dir_all(&package_path);
                return Err(error);
            }
            added.push((id, package_path));
        }
        Ok(())
    })();
    if let Err(error) = result {
        for id in enabled_existing.iter().rev() {
            let _ = super::undeploy_mod(&mut connection, &app_data, id, &root, true);
        }
        for (id, package_path) in added.iter().rev() {
            let _ = super::undeploy_mod(&mut connection, &app_data, id, &root, true);
            let _ = connection.execute("DELETE FROM mods WHERE id = ?1", [id]);
            let _ = fs::remove_dir_all(package_path);
        }
        return Err(error);
    }
    super::overview(&connection, &root, &game_id)
}

pub(crate) async fn install_mod(
    app: AppHandle,
    game_id: String,
    namespace: String,
    name: String,
    version: String,
) -> Result<ModManagerOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_sync(app, game_id, namespace, name, version)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bep_rules() -> GameRules {
        GameRules {
            community: "dusk".into(),
            name: "DUSK".into(),
            loader: "bepinex".into(),
            package_index: "https://thunderstore.io/c/dusk/api/v1/package-listing-index/".into(),
            install_rules: vec![
                InstallRule {
                    route: "BepInEx/plugins".into(),
                    default_file_extensions: vec![".dll".into()],
                    tracking_method: "subdir".into(),
                    sub_routes: vec![],
                    is_default_location: true,
                },
                InstallRule {
                    route: "BepInEx/config".into(),
                    default_file_extensions: vec![],
                    tracking_method: "none".into(),
                    sub_routes: vec![],
                    is_default_location: false,
                },
            ],
            exclusions: vec![],
        }
    }

    #[test]
    fn package_rules_route_plugins_configs_and_loader_files_for_the_game() {
        let rules = bep_rules();
        assert_eq!(
            map_package_path("SomePlugin.dll", &rules, "Author", "CoolMod").unwrap(),
            "BepInEx\\plugins\\Author-CoolMod\\SomePlugin.dll"
        );
        assert_eq!(
            map_package_path("config\\CoolMod.cfg", &rules, "Author", "CoolMod").unwrap(),
            "BepInEx\\config\\CoolMod.cfg"
        );
    }

    #[test]
    fn thunderstore_dependency_identifiers_split_from_the_right() {
        assert_eq!(
            split_dependency("Thunderstore-Team-Mod_Name-1.2.3").unwrap(),
            (
                "Thunderstore-Team".into(),
                "Mod_Name".into(),
                "1.2.3".into()
            )
        );
        assert!(split_dependency("../bad-thing-1.0.0").is_err());
    }
}
