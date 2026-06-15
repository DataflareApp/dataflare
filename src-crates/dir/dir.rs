use std::path::PathBuf;
use std::sync::LazyLock;

const IDENTIFIER: &str = "app.dataflare.desktop";
const DRIVER_DIR: &str = "drivers";
const CLIENT_DATABASE_FILE: &str = "data.db";
const THEME_FILE: &str = ".theme";
const WINDOW_STATE_FILE: &str = ".window-state";

const WINDOWS_PORTABLE_ROOT_DIR: &str = ".dataflare";
const WINDOWS_PORTABLE_WEBVIEW_DIR: &str = "webview";

static APP_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = if cfg!(feature = "portable") {
        match std::env::current_exe() {
            Ok(path) => match path.parent() {
                Some(parent) => parent.join(WINDOWS_PORTABLE_ROOT_DIR),
                None => exit("Executable directory not found"),
            },
            Err(err) => exit(err),
        }
    } else {
        match dirs::data_dir() {
            Some(path) => path.join(IDENTIFIER),
            None => exit("App data directory not found"),
        }
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
});

pub fn client_database_path() -> PathBuf {
    APP_DIR.join(CLIENT_DATABASE_FILE)
}

pub fn driver_dir() -> PathBuf {
    APP_DIR.join(DRIVER_DIR)
}

pub fn theme_path() -> PathBuf {
    APP_DIR.join(THEME_FILE)
}

pub fn window_state_path() -> PathBuf {
    APP_DIR.join(WINDOW_STATE_FILE)
}

pub fn webview_dir() -> PathBuf {
    APP_DIR.join(WINDOWS_PORTABLE_WEBVIEW_DIR)
}

fn exit(error: impl std::fmt::Display) -> ! {
    eprintln!("Error: {error}");
    std::process::exit(1);
}
