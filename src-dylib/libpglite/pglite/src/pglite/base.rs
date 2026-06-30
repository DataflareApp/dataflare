use std::fs;
use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};
use tempfile::{Builder, TempDir};

use crate::error::{Context, Result};

static EMBEDDED_RUNTIME: Dir<'static> = include_dir!("$OUT_DIR/pglite-runtime/tmp");
const PGLITE_WASI_PATH: &str = "pglite/bin/pglite.wasi";

pub fn runtime_module_bytes() -> Result<&'static [u8]> {
    EMBEDDED_RUNTIME
        .get_file(PGLITE_WASI_PATH)
        .map(|file| file.contents())
        .context("embedded runtime does not contain pglite/bin/pglite.wasi")
}

#[derive(Debug, Clone)]
pub struct PglitePaths {
    pub pgroot: PathBuf,
    pub pgdata: PathBuf,
}

impl PglitePaths {
    pub fn new(pgroot: impl Into<PathBuf>, pgdata: impl Into<PathBuf>) -> Self {
        let pgroot = pgroot.into();
        let pgdata = pgdata.into();
        Self { pgroot, pgdata }
    }

    pub fn is_cluster_initialized(&self) -> bool {
        self.pgdata.join("PG_VERSION").is_file()
    }
}

pub struct InstallOutcome {
    pub paths: PglitePaths,
    pub runtime_dir: TempDir,
}

pub fn install_into(pgdata: &Path) -> Result<InstallOutcome> {
    let runtime_dir = Builder::new()
        .prefix("pglite-runtime-")
        .tempdir()
        .context("create temporary PGlite runtime directory")?;
    let paths = PglitePaths::new(runtime_dir.path(), pgdata);
    install_runtime(&paths)?;
    fs::create_dir_all(&paths.pgdata)
        .with_context(|| format!("create PGDATA directory {}", paths.pgdata.display()))?;
    Ok(InstallOutcome { paths, runtime_dir })
}

fn install_runtime(paths: &PglitePaths) -> Result<()> {
    fs::create_dir_all(&paths.pgroot)
        .with_context(|| format!("create runtime directory {}", paths.pgroot.display()))?;
    EMBEDDED_RUNTIME
        .extract(&paths.pgroot)
        .with_context(|| format!("write embedded runtime to {}", paths.pgroot.display()))?;

    let module = paths.pgroot.join("pglite/bin/pglite.wasi");
    ensure!(
        module.is_file(),
        "embedded runtime did not contain {}",
        module.display()
    );
    Ok(())
}
