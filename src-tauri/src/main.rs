#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, fs, panic};

fn main() {
    panic::set_hook(Box::new(|info| {
        let base = env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| env::temp_dir())
            .join("Dusk");
        let _ = fs::create_dir_all(&base);
        let _ = fs::write(
            base.join("startup-panic.log"),
            format!("Dusk startup panic:\n{}\n", info),
        );
    }));

    dusk_lib::run();
}
