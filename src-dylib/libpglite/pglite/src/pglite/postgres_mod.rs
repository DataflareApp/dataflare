use crate::error::{Context, Result};
use getrandom::fill as fill_random;
use std::fmt;
use std::fs;
use std::sync::{LazyLock, Mutex};
use wasmtime::{
    Config, Engine, Instance, Linker, Memory, Module, OptLevel, Store, TypedFunc, WasmParams,
    WasmResults,
};
use wasmtime_wasi::p1::{WasiP1Ctx, add_to_linker_sync};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use super::base::{PglitePaths, runtime_module_bytes};

const WASM_PREFIX: &str = "/tmp/pglite";
const PGDATA_DIR: &str = "/tmp/pglite/base";

pub struct PostgresMod {
    _engine: Engine,
    store: Store<State>,
    _instance: Instance,
    memory: Memory,
    exports: Exports,
    paths: PglitePaths,
    transport: TransportMode,
    wire_enabled: bool,
}

enum TransportMode {
    Cma {
        buffer_addr: usize,
        buffer_len: usize,
    },
    File,
}

struct State {
    wasi: WasiP1Ctx,
}

static ENGINE: LazyLock<Engine> = LazyLock::new(build_engine);
static MODULE_CACHE: LazyLock<Mutex<Option<Module>>> = LazyLock::new(|| Mutex::new(None));

fn with_wasmtime_context<T>(
    result: std::result::Result<T, wasmtime::Error>,
    context: impl fmt::Display,
) -> Result<T> {
    result.map_err(|err| runtime_error!("{context}: {err}"))
}

fn build_engine() -> Engine {
    let mut config = Config::new();

    config.cranelift_opt_level(OptLevel::None);

    Engine::new(&config).expect("failed to create Wasmtime engine")
}

fn load_module() -> Result<(Engine, Module)> {
    let engine = ENGINE.clone();
    let mut cached = MODULE_CACHE
        .lock()
        .map_err(|err| runtime_error!("module cache lock poisoned: {err}"))?;
    if let Some(module) = cached.as_ref() {
        return Ok((engine, module.clone()));
    }

    let module = with_wasmtime_context(
        Module::from_binary(&engine, runtime_module_bytes()?),
        "failed to compile embedded pglite.wasi",
    )?;
    *cached = Some(module.clone());
    Ok((engine, module))
}

struct Exports {
    pgl_initdb: TypedFunc<(), i32>,
    pgl_backend: TypedFunc<(), ()>,
    pgl_shutdown: TypedFunc<(), ()>,
    use_wire: TypedFunc<i32, ()>,
    interactive_write: TypedFunc<i32, ()>,
    interactive_one: TypedFunc<(), ()>,
    interactive_read: TypedFunc<(), i32>,
    get_channel: TypedFunc<(), i32>,
    get_buffer_size: TypedFunc<i32, i32>,
    get_buffer_addr: TypedFunc<i32, i32>,
}

impl PostgresMod {
    pub fn new(paths: PglitePaths, database: &str) -> Result<Self> {
        let module_path = paths.pgroot.join("pglite/bin/pglite.wasi");

        if !module_path.exists() {
            return Err(runtime_error!(
                "pglite.wasi binary not found at {}",
                module_path.display()
            ));
        }

        let (engine, module) = load_module()?;

        let mut linker: Linker<State> = Linker::new(&engine);
        with_wasmtime_context(
            add_to_linker_sync(&mut linker, |state| &mut state.wasi),
            "failed to add WASI to linker",
        )?;

        let wasi = build_wasi_ctx(&paths, database)?;
        let mut store = Store::new(&engine, State { wasi });

        let instance = with_wasmtime_context(
            linker.instantiate(&mut store, &module),
            "failed to instantiate pglite module",
        )?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("pglite module is missing exported memory")?;

        if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "_start")
            && let Err(_err) = start.call(&mut store, ())
        {
            // _start trapped during startup and was ignored: {err}
        }

        let exports = Exports::load(&mut store, &instance)?;

        let channel_id = with_wasmtime_context(
            exports.get_channel.call(&mut store, ()),
            "call _get_channel",
        )?;
        let transport = if channel_id >= 0 {
            let addr = with_wasmtime_context(
                exports.get_buffer_addr.call(&mut store, channel_id),
                "call _get_buffer_addr",
            )?;
            let len = with_wasmtime_context(
                exports.get_buffer_size.call(&mut store, channel_id),
                "call _get_buffer_size",
            )?;
            ensure!(addr >= 0, "interactive buffer address is negative: {addr}");
            ensure!(len >= 0, "interactive buffer length is negative: {len}");
            TransportMode::Cma {
                buffer_addr: addr as usize,
                buffer_len: len as usize,
            }
        } else {
            TransportMode::File
        };

        Ok(Self {
            _engine: engine,
            store,
            _instance: instance,
            memory,
            exports,
            paths,
            transport,
            wire_enabled: false,
        })
    }

    pub fn paths(&self) -> &PglitePaths {
        &self.paths
    }

    pub fn ensure_cluster(&mut self) -> Result<()> {
        let had_cluster = self.paths.is_cluster_initialized();
        // PGlite uses this export for runtime setup as well as first-time
        // cluster creation, so existing clusters still need the call.
        let rc = self
            .exports
            .pgl_initdb
            .call(&mut self.store, ())
            .map_err(|err| runtime_error!("failed to execute _pgl_initdb: {err}"))?;

        if rc != 0 {
            if self.paths.is_cluster_initialized() {
                if !had_cluster {
                    // _pgl_initdb returned status {rc}, but PG_VERSION exists; continuing
                }
                return Ok(());
            }
            return Err(runtime_error!(
                "_pgl_initdb returned non-zero status: {}",
                rc
            ));
        }

        if !self.paths.is_cluster_initialized() {
            return Err(runtime_error!(
                "_pgl_initdb returned success but PG_VERSION is missing"
            ));
        }

        Ok(())
    }

    pub fn buffer_addr(&self) -> Option<usize> {
        match self.transport {
            TransportMode::Cma { buffer_addr, .. } => Some(buffer_addr),
            TransportMode::File => None,
        }
    }

    pub fn buffer_len(&self) -> Option<usize> {
        match self.transport {
            TransportMode::Cma { buffer_len, .. } => Some(buffer_len),
            TransportMode::File => None,
        }
    }

    pub fn write_memory(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        self.memory
            .write(&mut self.store, offset, data)
            .with_context(|| format!("write {} bytes at 0x{offset:x}", data.len()))
    }

    pub fn read_memory(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.memory
            .read(&mut self.store, offset, buf)
            .with_context(|| format!("read {} bytes at 0x{offset:x}", buf.len()))
    }

    pub fn interactive_write(&mut self, len: i32) -> Result<()> {
        self.exports
            .interactive_write
            .call(&mut self.store, len)
            .map_err(|err| runtime_error!("call _interactive_write: {err}"))?;
        Ok(())
    }

    pub fn interactive_one(&mut self) -> Result<()> {
        self.exports
            .interactive_one
            .call(&mut self.store, ())
            .map_err(|err| runtime_error!("call _interactive_one: {err}"))?;
        Ok(())
    }

    pub fn interactive_read(&mut self) -> Result<i32> {
        self.exports
            .interactive_read
            .call(&mut self.store, ())
            .map_err(|err| runtime_error!("call _interactive_read: {err}"))
    }

    pub fn use_wire(&mut self, enabled: bool) -> Result<()> {
        self.exports
            .use_wire
            .call(&mut self.store, if enabled { 1 } else { 0 })
            .map_err(|err| runtime_error!("call _use_wire: {err}"))?;
        self.wire_enabled = enabled;
        Ok(())
    }

    pub fn backend(&mut self) -> Result<()> {
        self.exports
            .pgl_backend
            .call(&mut self.store, ())
            .map_err(|err| runtime_error!("call _pgl_backend: {err}"))?;
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.exports
            .pgl_shutdown
            .call(&mut self.store, ())
            .map_err(|err| runtime_error!("call _pgl_shutdown: {err}"))
    }
}

impl Exports {
    fn load(store: &mut Store<State>, instance: &Instance) -> Result<Self> {
        fn get_typed<P, R>(
            store: &mut Store<State>,
            instance: &Instance,
            names: &[&str],
        ) -> Result<TypedFunc<P, R>>
        where
            P: WasmParams,
            R: WasmResults,
        {
            for name in names {
                if let Ok(func) = instance.get_typed_func::<P, R>(&mut *store, name) {
                    return Ok(func);
                }
            }
            bail!("missing expected export {:?}", names)
        }

        let pgl_initdb = get_typed(store, instance, &["_pgl_initdb", "pgl_initdb"])?;
        let pgl_backend = get_typed(store, instance, &["_pgl_backend", "pgl_backend"])?;
        let pgl_shutdown = get_typed(store, instance, &["_pgl_shutdown", "pgl_shutdown"])?;
        let use_wire = get_typed(store, instance, &["_use_wire", "use_wire"])?;
        let interactive_write = get_typed(
            store,
            instance,
            &["_interactive_write", "interactive_write"],
        )?;
        let interactive_one = get_typed(store, instance, &["_interactive_one", "interactive_one"])?;
        let interactive_read =
            get_typed(store, instance, &["_interactive_read", "interactive_read"])?;
        let get_channel = get_typed(store, instance, &["_get_channel", "get_channel"])?;
        let get_buffer_size = get_typed(store, instance, &["_get_buffer_size", "get_buffer_size"])?;
        let get_buffer_addr = get_typed(store, instance, &["_get_buffer_addr", "get_buffer_addr"])?;

        Ok(Self {
            pgl_initdb,
            pgl_backend,
            pgl_shutdown,
            use_wire,
            interactive_write,
            interactive_one,
            interactive_read,
            get_channel,
            get_buffer_size,
            get_buffer_addr,
        })
    }
}

fn build_wasi_ctx(paths: &PglitePaths, database: &str) -> Result<WasiP1Ctx> {
    ensure_runtime_dirs(paths)?;

    let mut builder = WasiCtxBuilder::new();

    builder
        .env("PREFIX", WASM_PREFIX)
        .env("PGDATA", PGDATA_DIR)
        .env("PGUSER", "postgres")
        .env("PGDATABASE", database)
        .env("MODE", "REACT")
        .env("REPL", "N")
        .env("PGSYSCONFDIR", WASM_PREFIX)
        .env("PGCLIENTENCODING", "UTF8")
        .env("LC_CTYPE", "C.UTF-8")
        .env("TZ", "UTC")
        .env("PGTZ", "UTC")
        .env("PG_COLOR", "never");

    builder.arg(format!("PGDATA={}", PGDATA_DIR));
    builder.arg(format!("PREFIX={}", WASM_PREFIX));
    builder.arg("PGUSER=postgres");
    builder.arg(format!("PGDATABASE={database}"));
    builder.arg("MODE=REACT");
    builder.arg("REPL=N");

    let host_tmp = paths.pgroot.clone();
    builder
        .preopened_dir(&host_tmp, "/tmp", DirPerms::all(), FilePerms::all())
        .map_err(|err| runtime_error!("failed to preopen {} as /tmp: {err}", host_tmp.display()))?;

    let home_path = paths.pgroot.join("home");
    if !home_path.exists() {
        fs::create_dir_all(&home_path)
            .with_context(|| format!("failed to create {}", home_path.display()))?;
    }
    builder
        .preopened_dir(&home_path, "/home", DirPerms::all(), FilePerms::all())
        .map_err(|err| {
            runtime_error!("failed to preopen {} as /home: {err}", home_path.display())
        })?;

    builder
        .preopened_dir(
            &paths.pgdata,
            "/tmp/pglite/base",
            DirPerms::all(),
            FilePerms::all(),
        )
        .map_err(|err| {
            runtime_error!(
                "failed to preopen {} as /tmp/pglite/base: {err}",
                paths.pgdata.display()
            )
        })?;

    let dev_path = paths.pgroot.join("dev");
    builder
        .preopened_dir(&dev_path, "/dev", DirPerms::all(), FilePerms::all())
        .map_err(|err| runtime_error!("failed to preopen {} as /dev: {err}", dev_path.display()))?;

    Ok(builder.build_p1())
}

fn ensure_runtime_dirs(paths: &PglitePaths) -> Result<()> {
    let dev_path = paths.pgroot.join("dev");
    if !dev_path.exists() {
        fs::create_dir_all(&dev_path)
            .with_context(|| format!("failed to create {}", dev_path.display()))?;
    }
    let urandom = dev_path.join("urandom");
    if !urandom.exists() {
        let mut buf = [0u8; 128];
        fill_random(&mut buf).context("seed urandom")?;
        fs::write(&urandom, buf)
            .with_context(|| format!("failed to seed {}", urandom.display()))?;
    }

    if !paths.pgdata.exists() {
        fs::create_dir_all(&paths.pgdata)
            .with_context(|| format!("failed to create {}", paths.pgdata.display()))?;
    }

    Ok(())
}
