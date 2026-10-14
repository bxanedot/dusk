use quick_xml::{escape::unescape, events::Event, Reader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};
use tauri::AppHandle;

use super::{app_data_dir, get_game, open_database};

const MAX_TABLE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_TABLE_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheatEntry {
    pub id: String,
    pub title: String,
    pub address: String,
    pub variable_type: String,
    pub value: String,
    pub show_as_hex: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheatTable {
    pub source_file_name: String,
    pub imported_at: String,
    pub skipped_entries: usize,
    pub entries: Vec<CheatEntry>,
}

#[derive(Default)]
struct EntryBuilder {
    index: usize,
    title: String,
    address: String,
    variable_type: String,
    value: String,
    show_as_hex: bool,
    current_field: Option<String>,
    has_child_entries: bool,
    unsupported: bool,
}

static ACTIVE_HOLDS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn active_holds() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_HOLDS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn game_key(game_id: &str) -> String {
    Sha256::digest(game_id.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn table_path(app: &AppHandle, game_id: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?
        .join("cheat-tables")
        .join(format!("{}.json", game_key(game_id))))
}

fn read_table(app: &AppHandle, game_id: &str) -> Result<Option<CheatTable>, String> {
    let path = table_path(app, game_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Could not read this game's table: {error}"))?;
    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|error| format!("This game's saved table is invalid: {error}"))
}

fn write_table(app: &AppHandle, game_id: &str, table: &CheatTable) -> Result<(), String> {
    let path = table_path(app, game_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Could not find the table storage folder.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create the table storage folder: {error}"))?;
    let contents = serde_json::to_vec_pretty(table).map_err(|error| error.to_string())?;
    fs::write(&path, contents).map_err(|error| format!("Could not save the table: {error}"))
}

fn set_field(entry: &mut EntryBuilder, name: &str, value: &str) {
    let value = value.trim();
    match name {
        "Description" => entry.title.push_str(value),
        "Address" => entry.address.push_str(value),
        "VariableType" => entry.variable_type.push_str(value),
        "Value" => entry.value.push_str(value),
        "ShowAsHex" => entry.show_as_hex = value == "1" || value.eq_ignore_ascii_case("true"),
        _ => {}
    }
}

fn supported_value_bytes(entry: &CheatEntry) -> Result<Vec<u8>, String> {
    let value = entry.value.trim();
    let kind = entry.variable_type.trim().to_ascii_lowercase();
    match kind.as_str() {
        "float" => {
            let parsed: f32 = value
                .parse()
                .map_err(|_| "The table has an invalid float value.".to_string())?;
            if !parsed.is_finite() {
                return Err("Non-finite float values are not supported.".into());
            }
            Ok(parsed.to_le_bytes().to_vec())
        }
        "double" => {
            let parsed: f64 = value
                .parse()
                .map_err(|_| "The table has an invalid double value.".to_string())?;
            if !parsed.is_finite() {
                return Err("Non-finite double values are not supported.".into());
            }
            Ok(parsed.to_le_bytes().to_vec())
        }
        "byte" | "1 byte" | "2 bytes" | "4 bytes" | "8 bytes" => {
            let byte_count = match kind.as_str() {
                "byte" | "1 byte" => 1,
                "2 bytes" => 2,
                "4 bytes" => 4,
                _ => 8,
            };
            let parsed = if entry.show_as_hex {
                let normalized = value.trim_start_matches("0x").trim_start_matches("0X");
                i128::from_str_radix(normalized, 16)
                    .map_err(|_| "The table has an invalid hexadecimal value.".to_string())?
            } else {
                value
                    .parse::<i128>()
                    .map_err(|_| "The table has an invalid integer value.".to_string())?
            };
            let bits = (byte_count * 8) as u32;
            let min = -(1_i128 << (bits - 1));
            let max_unsigned = (1_i128 << bits) - 1;
            if parsed < min || parsed > max_unsigned {
                return Err("The table value does not fit its declared number type.".into());
            }
            let full = (parsed as u128).to_le_bytes();
            Ok(full[..byte_count].to_vec())
        }
        _ => Err("This table entry uses a type Dusk does not support.".into()),
    }
}

fn valid_address_syntax(address: &str) -> bool {
    let address = address.trim();
    if address.is_empty()
        || address
            .chars()
            .any(|character| matches!(character, '[' | ']' | '*' | '/' | '\\'))
    {
        return false;
    }
    if let Some((module, offset)) = address.split_once('+') {
        !module.trim().is_empty()
            && module.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            })
            && parse_hex(offset).is_some()
            && !offset.contains('+')
    } else {
        parse_hex(address).is_some()
    }
}

fn parse_hex(value: &str) -> Option<usize> {
    let normalized = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }
    usize::from_str_radix(normalized, 16).ok()
}

fn finish_entry(builder: EntryBuilder, output: &mut Vec<CheatEntry>, skipped: &mut usize) {
    if builder.has_child_entries {
        return;
    }
    let mut title = builder.title.trim().trim_matches('"').trim().to_string();
    if title.is_empty() {
        title = "Untitled value".into();
    }
    let entry = CheatEntry {
        id: format!("entry-{}", builder.index),
        title,
        address: builder.address.trim().to_string(),
        variable_type: builder.variable_type.trim().to_string(),
        value: builder.value.trim().to_string(),
        show_as_hex: builder.show_as_hex,
    };
    if builder.unsupported
        || !valid_address_syntax(&entry.address)
        || supported_value_bytes(&entry).is_err()
    {
        *skipped += 1;
        return;
    }
    output.push(entry);
}

fn parse_table(contents: &str) -> Result<(Vec<CheatEntry>, usize), String> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(true);
    let mut element_path: Vec<String> = Vec::new();
    let mut entries: Vec<EntryBuilder> = Vec::new();
    let mut parsed_entries = Vec::new();
    let mut skipped = 0;
    let mut seen = 0usize;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("Could not parse the table XML: {error}"))?;
        match event {
            Event::Start(tag) => {
                let name = tag.local_name().as_ref().to_string();
                if name == "CheatEntry" {
                    if let Some(parent) = entries.last_mut() {
                        parent.has_child_entries = true;
                    }
                    seen += 1;
                    if seen > MAX_TABLE_ENTRIES {
                        return Err(format!(
                            "This table contains more than {MAX_TABLE_ENTRIES} entries."
                        ));
                    }
                    entries.push(EntryBuilder {
                        index: seen,
                        ..EntryBuilder::default()
                    });
                } else if let Some(entry) = entries.last_mut() {
                    if element_path.last().map(String::as_str) == Some("CheatEntry") {
                        if matches!(
                            name.as_str(),
                            "Description" | "Address" | "VariableType" | "Value" | "ShowAsHex"
                        ) {
                            entry.current_field = Some(name.clone());
                        }
                    }
                    if matches!(
                        name.as_str(),
                        "AssemblerScript"
                            | "LuaScript"
                            | "AutoAssembler"
                            | "Offsets"
                            | "Offset"
                            | "CustomType"
                            | "VariableLink"
                    ) {
                        entry.unsupported = true;
                    }
                }
                element_path.push(name);
            }
            Event::Empty(tag) => {
                let name = tag.local_name().as_ref().to_string();
                if let Some(entry) = entries.last_mut() {
                    if matches!(
                        name.as_str(),
                        "AssemblerScript"
                            | "LuaScript"
                            | "AutoAssembler"
                            | "Offsets"
                            | "Offset"
                            | "CustomType"
                            | "VariableLink"
                    ) {
                        entry.unsupported = true;
                    }
                }
            }
            Event::Text(text) => {
                if let (Some(entry), Some(field)) = (entries.last_mut(), element_path.last()) {
                    if let Some(current) = entry.current_field.clone() {
                        if current == *field {
                            let normalized = text.xml10_content();
                            let decoded = unescape(normalized.as_ref()).map_err(|error| {
                                format!("Could not decode a table value: {error}")
                            })?;
                            set_field(entry, &current, decoded.as_ref());
                        }
                    }
                }
            }
            Event::CData(text) => {
                if let (Some(entry), Some(field)) = (entries.last_mut(), element_path.last()) {
                    if let Some(current) = entry.current_field.clone() {
                        if current == *field {
                            set_field(entry, &current, text.xml10_content().as_ref());
                        }
                    }
                }
            }
            Event::End(tag) => {
                let name = tag.local_name().as_ref().to_string();
                if name == "CheatEntry" {
                    if let Some(entry) = entries.pop() {
                        finish_entry(entry, &mut parsed_entries, &mut skipped);
                    }
                } else if let Some(entry) = entries.last_mut() {
                    if entry.current_field.as_deref() == Some(name.as_str()) {
                        entry.current_field = None;
                    }
                }
                element_path.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !entries.is_empty() {
        return Err("The table XML ended inside a cheat entry.".into());
    }
    if parsed_entries.is_empty() {
        return Err("No supported numeric entries were found. Script, pointer, and custom-type entries are not loaded.".into());
    }
    Ok((parsed_entries, skipped))
}

#[tauri::command]
pub fn get_cheat_table(app: AppHandle, game_id: String) -> Result<Option<CheatTable>, String> {
    read_table(&app, &game_id)
}

#[tauri::command]
pub fn import_cheat_table(app: AppHandle, game_id: String) -> Result<Option<CheatTable>, String> {
    let Some(source) = rfd::FileDialog::new()
        .add_filter("Cheat tables", &["ct"])
        .pick_file()
    else {
        return Ok(None);
    };
    let metadata = fs::metadata(&source)
        .map_err(|error| format!("Could not inspect the table file: {error}"))?;
    if metadata.len() > MAX_TABLE_BYTES {
        return Err("Table files must be 10 MB or smaller.".into());
    }
    let contents = fs::read_to_string(&source)
        .map_err(|error| format!("Could not read the table as UTF-8 XML: {error}"))?;
    let (entries, skipped_entries) = parse_table(&contents)?;
    let table = CheatTable {
        source_file_name: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        imported_at: chrono::Utc::now().to_rfc3339(),
        skipped_entries,
        entries,
    };
    write_table(&app, &game_id, &table)?;
    Ok(Some(table))
}

fn game_executable(app: &AppHandle, game_id: &str) -> Result<PathBuf, String> {
    let connection = open_database(app)?;
    let game = get_game(&connection, game_id)?;
    let executable = game.exe_path.ok_or_else(|| {
        "Dusk has no executable path for this game. Add the game's .exe in the library first."
            .to_string()
    })?;
    let path = PathBuf::from(executable);
    path.canonicalize()
        .map_err(|error| format!("Could not resolve the game's executable: {error}"))
}

fn selected_entry(
    app: &AppHandle,
    game_id: &str,
    entry_id: &str,
) -> Result<(PathBuf, CheatEntry), String> {
    let executable = game_executable(app, game_id)?;
    let table = read_table(app, game_id)?
        .ok_or_else(|| "Import a .CT table for this game first.".to_string())?;
    let entry = table
        .entries
        .into_iter()
        .find(|entry| entry.id == entry_id)
        .ok_or_else(|| {
            "This entry is no longer in the imported table. Reload the game details.".to_string()
        })?;
    Ok((executable, entry))
}

#[tauri::command]
pub fn apply_cheat_value(app: AppHandle, game_id: String, entry_id: String) -> Result<(), String> {
    let (executable, entry) = selected_entry(&app, &game_id, &entry_id)?;
    let bytes = supported_value_bytes(&entry)?;
    write_game_value(&executable, &entry.address, &bytes)
}

#[tauri::command]
pub fn set_cheat_hold(
    app: AppHandle,
    game_id: String,
    entry_id: String,
    enabled: bool,
) -> Result<(), String> {
    let key = format!("{}:{}", game_key(&game_id), entry_id);
    if !enabled {
        if let Some(stop) = active_holds()
            .lock()
            .map_err(|_| "Trainer state is unavailable.".to_string())?
            .remove(&key)
        {
            stop.store(false, Ordering::Relaxed);
        }
        return Ok(());
    }
    if active_holds()
        .lock()
        .map_err(|_| "Trainer state is unavailable.".to_string())?
        .contains_key(&key)
    {
        return Ok(());
    }

    let (executable, entry) = selected_entry(&app, &game_id, &entry_id)?;
    let bytes = supported_value_bytes(&entry)?;
    let (process_id, address, expected_path) = prepare_game_value(&executable, &entry.address)?;
    write_game_value_for_process(process_id, &expected_path, address, &bytes)?;

    let stop = Arc::new(AtomicBool::new(true));
    {
        let mut active = active_holds()
            .lock()
            .map_err(|_| "Trainer state is unavailable.".to_string())?;
        if active.contains_key(&key) {
            return Ok(());
        }
        active.insert(key.clone(), stop.clone());
    }
    thread::spawn(move || {
        while stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(450));
            if !stop.load(Ordering::Relaxed) {
                break;
            }
            if write_game_value_for_process(process_id, &expected_path, address, &bytes).is_err() {
                break;
            }
        }
        stop.store(false, Ordering::Relaxed);
        if let Ok(mut active) = active_holds().lock() {
            if active
                .get(&key)
                .is_some_and(|current| Arc::ptr_eq(current, &stop))
            {
                active.remove(&key);
            }
        }
    });
    Ok(())
}

#[cfg(target_os = "windows")]
fn write_game_value(executable: &Path, address: &str, bytes: &[u8]) -> Result<(), String> {
    let (process_id, target_address, expected_path) = prepare_game_value(executable, address)?;
    write_game_value_for_process(process_id, &expected_path, target_address, bytes)
}

#[cfg(not(target_os = "windows"))]
fn write_game_value(_executable: &Path, _address: &str, _bytes: &[u8]) -> Result<(), String> {
    Err("Trainer memory access is currently available on Windows only.".into())
}

#[cfg(target_os = "windows")]
fn prepare_game_value(executable: &Path, address: &str) -> Result<(u32, usize, String), String> {
    let expected_path = canonical_path_key(executable)?;
    let process_id = find_exact_process(executable, &expected_path)?;
    let target_address = resolve_address(process_id, address)?;
    Ok((process_id, target_address, expected_path))
}

#[cfg(not(target_os = "windows"))]
fn prepare_game_value(_executable: &Path, _address: &str) -> Result<(u32, usize, String), String> {
    Err("Trainer memory access is currently available on Windows only.".into())
}

#[cfg(target_os = "windows")]
fn write_game_value_for_process(
    process_id: u32,
    expected_path: &str,
    address: usize,
    bytes: &[u8],
) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::Debug::WriteProcessMemory,
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_OPERATION,
                PROCESS_VM_WRITE,
            },
        },
    };
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE,
            0,
            process_id,
        )
    };
    if handle.is_null() {
        return Err("Could not open the game process. Try running Dusk with the same permission level as the game.".into());
    }
    let result = verify_process_path(handle, expected_path).and_then(|_| {
        let mut written = 0usize;
        let okay = unsafe {
            WriteProcessMemory(handle, address as *const _, bytes.as_ptr() as *const _, bytes.len(), &mut written)
        };
        if okay == 0 || written != bytes.len() {
            Err("Windows could not write this value. The game may have changed version or denied access.".into())
        } else {
            Ok(())
        }
    });
    unsafe {
        CloseHandle(handle);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn write_game_value_for_process(
    _process_id: u32,
    _expected_path: &str,
    _address: usize,
    _bytes: &[u8],
) -> Result<(), String> {
    Err("Trainer memory access is currently available on Windows only.".into())
}

#[cfg(target_os = "windows")]
fn canonical_path_key(path: &Path) -> Result<String, String> {
    path.canonicalize()
        .map(|path| path.to_string_lossy().to_ascii_lowercase())
        .map_err(|error| format!("Could not resolve the executable path: {error}"))
}

#[cfg(target_os = "windows")]
fn wide_string(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

#[cfg(target_os = "windows")]
fn query_process_path(handle: windows_sys::Win32::Foundation::HANDLE) -> Result<String, String> {
    use windows_sys::Win32::System::Threading::{QueryFullProcessImageNameW, PROCESS_NAME_WIN32};
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    let okay = unsafe {
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut length)
    };
    if okay == 0 {
        return Err("Could not read the running game's executable path.".into());
    }
    Ok(String::from_utf16_lossy(&buffer[..length as usize]).to_ascii_lowercase())
}

#[cfg(target_os = "windows")]
fn verify_process_path(
    handle: windows_sys::Win32::Foundation::HANDLE,
    expected_path: &str,
) -> Result<(), String> {
    let actual = query_process_path(handle)?;
    let canonical_actual = Path::new(&actual)
        .canonicalize()
        .map(|path| path.to_string_lossy().to_ascii_lowercase())
        .unwrap_or(actual);
    if canonical_actual == expected_path {
        Ok(())
    } else {
        Err("The selected process is not the executable registered for this game.".into())
    }
}

#[cfg(target_os = "windows")]
fn find_exact_process(executable: &Path, expected_path: &str) -> Result<u32, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("Could not inspect running game processes.".into());
    }
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut found = None;
    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while has_entry {
        if wide_string(&entry.szExeFile).eq_ignore_ascii_case(
            executable
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        ) {
            let handle =
                unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID) };
            if !handle.is_null() {
                if verify_process_path(handle, expected_path).is_ok() {
                    found = Some(entry.th32ProcessID);
                    unsafe {
                        CloseHandle(handle);
                    }
                    break;
                }
                unsafe {
                    CloseHandle(handle);
                }
            }
        }
        has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe {
        CloseHandle(snapshot);
    }
    found.ok_or_else(|| "Start this game before applying a value or enabling Hold.".into())
}

#[cfg(target_os = "windows")]
fn resolve_address(process_id: u32, address: &str) -> Result<usize, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W,
            TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        },
    };
    let address = address.trim();
    if let Some((module_name, offset_text)) = address.split_once('+') {
        let offset = parse_hex(offset_text)
            .ok_or_else(|| "This table has an invalid module offset.".to_string())?;
        let snapshot = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id)
        };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("Could not inspect the game's loaded modules.".into());
        }
        let mut module = MODULEENTRY32W::default();
        module.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut base = None;
        let mut has_module = unsafe { Module32FirstW(snapshot, &mut module) } != 0;
        while has_module {
            if wide_string(&module.szModule).eq_ignore_ascii_case(module_name.trim()) {
                base = Some(module.modBaseAddr as usize);
                break;
            }
            has_module = unsafe { Module32NextW(snapshot, &mut module) } != 0;
        }
        unsafe {
            CloseHandle(snapshot);
        }
        let base = base.ok_or_else(|| {
            format!("The table module '{module_name}' is not loaded in this game.")
        })?;
        base.checked_add(offset)
            .ok_or_else(|| "The table address is outside the valid address range.".into())
    } else {
        parse_hex(address).ok_or_else(|| "This table has an invalid absolute address.".into())
    }
}
