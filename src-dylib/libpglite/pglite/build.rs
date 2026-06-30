use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use xz2::read::XzDecoder;

// https://github.com/electric-sql/pglite-build/commits/gh-pages/
const PGLITE_BUILD_COMMIT: &str = "4c78ee29513799a51d4e1f75008cf9c3f00b11e9";
const RUNTIME_SHA256: &str = "c725235f22a4fd50fed363f4065edb151a716fa769cba66f2383b8b854e6bdb5";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let out = env::var_os("OUT_DIR").unwrap();
    let out_dir = PathBuf::from(out);
    let runtime = out_dir.join("pglite-wasi.tar.xz");
    if !asset_is_valid(&runtime, RUNTIME_SHA256) {
        download_runtime(&runtime);
    }
    extract_runtime(&runtime, &out_dir.join("pglite-runtime"));
}

fn extract_runtime(archive_path: &Path, destination: &Path) {
    let marker = destination.join(".runtime-sha256");
    if fs::read_to_string(&marker).is_ok_and(|value| value == RUNTIME_SHA256) {
        return;
    }

    let temporary = destination.with_extension("tmp");
    if temporary.exists() {
        fs::remove_dir_all(&temporary).unwrap();
    }
    fs::create_dir_all(&temporary).unwrap();

    let archive = File::open(archive_path).unwrap();
    let decoder = XzDecoder::new(archive);
    Archive::new(decoder).unpack(&temporary).unwrap();

    if destination.exists() {
        fs::remove_dir_all(destination).unwrap();
    }
    fs::write(temporary.join(".runtime-sha256"), RUNTIME_SHA256).unwrap();
    fs::rename(&temporary, destination).unwrap();
}

fn download_runtime(runtime: &Path) {
    let url = format!(
        "https://raw.githubusercontent.com/electric-sql/pglite-build/{PGLITE_BUILD_COMMIT}/pglite-wasi.tar.xz"
    );
    let temporary = runtime.with_extension("xz.tmp");
    let status = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .arg(&url)
        .arg("--output")
        .arg(&temporary)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(asset_is_valid(&temporary, RUNTIME_SHA256));
    fs::rename(&temporary, runtime).unwrap();
}

fn asset_is_valid(path: &Path, expected: &str) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let actual = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    actual == expected
}
