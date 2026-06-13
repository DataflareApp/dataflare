use std::path::PathBuf;

const IDENTIFIER: &str = "app.dataflare.desktop";
#[cfg(all(target_os = "windows", feature = "portable"))]
const PORTABLE_DATA_DIR: &str = ".dataflare";
const DRIVER_DIR: &str = "drivers";
#[cfg(all(target_os = "windows", feature = "portable"))]
pub const WEBVIEW_DIR: &str = "webview";
pub const CLIENT_DATABASE_FILE: &str = "data.db";
pub const THEME_FILE: &str = ".theme";
pub const WINDOW_STATE_FILE: &str = ".window-state";

pub fn app_dir() -> PathBuf {
    #[cfg(all(target_os = "windows", feature = "portable"))]
    {
        let executable = std::env::current_exe().unwrap_or_else(|err| exit(err));
        let parent = executable
            .parent()
            .unwrap_or_else(|| exit("Executable directory not found"));
        return parent.join(PORTABLE_DATA_DIR);
    }

    #[cfg(not(all(target_os = "windows", feature = "portable")))]
    dirs::data_dir()
        .map(|path| path.join(IDENTIFIER))
        .unwrap_or_else(|| exit("App data directory not found"))
}

pub fn driver_dir() -> PathBuf {
    app_dir().join(DRIVER_DIR)
}

fn exit(error: impl std::fmt::Display) -> ! {
    eprintln!("Error: {error}");
    std::process::exit(1);
}
