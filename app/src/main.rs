#[cfg(target_os = "macos")]
mod mac;
mod shell;

use cef::*;
use std::path::PathBuf;

fn main() {
    #[cfg(target_os = "macos")]
    let _loader = {
        let loader = library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), false);
        assert!(loader.load());
        loader
    };
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    #[cfg(target_os = "macos")]
    mac::setup_application();

    let args = args::Args::new();
    if execute_process(Some(args.as_main_args()), None, std::ptr::null_mut()) >= 0 {
        return;
    }

    let root = data_dir();
    let settings = Settings {
        root_cache_path: CefString::from(root.to_str().unwrap()),
        cache_path: CefString::from(root.join("Default").to_str().unwrap()),
        ..Default::default()
    };
    let mut app = shell::ArbaletteApp::new();
    assert_eq!(
        initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()),
        1
    );
    #[cfg(target_os = "macos")]
    mac::setup_menu();

    run_message_loop();
    shutdown();
}

/// Where Arbalette keeps its data, resolved because CEF compares paths after resolving symlinks.
fn data_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap());
    let dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Arbalette")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("Arbalette")
    } else {
        std::env::var_os("XDG_DATA_HOME").map_or_else(|| home.join(".local/share"), PathBuf::from).join("arbalette")
    };
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}
