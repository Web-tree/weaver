use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable, bindgen};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

bindgen!({
    world: "ensure-provider",
    path: "../../wit",
});

pub struct EnsureHost {
    pub table: ResourceTable,
    pub wasi: WasiCtx,
}

impl EnsureHost {
    pub fn new() -> Self {
        // Build WASI context with inherited environment
        let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_env().build();

        Self {
            table: ResourceTable::new(),
            wasi,
        }
    }
}

impl WasiView for EnsureHost {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        wasmtime_wasi::WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl weaver::plugin::process::Host for EnsureHost {
    fn exec(
        &mut self,
        req: weaver::plugin::process::ExecRequest,
    ) -> Result<weaver::plugin::process::ExecResult, String> {
        let mut cmd = Command::new(&req.program);
        cmd.args(&req.args);

        if let Some(cwd) = &req.cwd {
            cmd.current_dir(cwd);
        }

        if !req.inherit_env {
            cmd.env_clear();
        }

        for (k, v) in &req.env {
            cmd.env(k, v);
        }

        cmd.stdin(if req.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to spawn {}: {}", req.program, e)),
        };

        if let Some(input) = &req.stdin {
            if let Some(mut s) = child.stdin.take() {
                if let Err(e) = s.write_all(input) {
                    return Err(format!("Failed to write stdin: {}", e));
                }
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait on child: {}", e))?;
        let code = output.status.code().unwrap_or(1) as u32;

        Ok(weaver::plugin::process::ExecResult {
            status: code,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

pub struct EnsurePluginEngine {
    engine: Engine,
    linker: Linker<EnsureHost>,
}

impl EnsurePluginEngine {
    pub fn new() -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;

        let mut linker: Linker<EnsureHost> = Linker::new(&engine);

        // Add WASI support (required by cargo-component built plugins)
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

        // Add our custom ensure-provider interface
        EnsureProvider::add_to_linker::<EnsureHost, HasSelf<EnsureHost>>(&mut linker, |state| {
            state
        })?;

        Ok(Self { engine, linker })
    }

    pub fn load_plugin(&self, wasm_path: &Path) -> anyhow::Result<LoadedEnsurePlugin> {
        let component = Component::from_file(&self.engine, wasm_path).map_err(|e| {
            super::PluginError::InvalidWasm {
                path: wasm_path.display().to_string(),
                source: e.into(),
            }
        })?;
        Ok(LoadedEnsurePlugin {
            engine: self.engine.clone(),
            linker: self.linker.clone(),
            component,
        })
    }
}

pub struct LoadedEnsurePlugin {
    engine: Engine,
    linker: Linker<EnsureHost>,
    component: Component,
}

impl LoadedEnsurePlugin {
    pub fn plan(
        &self,
        app_path: &str,
        dry_run: bool,
        config_json: &str,
    ) -> anyhow::Result<exports::weaver::plugin::ensures::EnsurePlan> {
        let mut store = Store::new(&self.engine, EnsureHost::new());
        let bindings = EnsureProvider::instantiate(&mut store, &self.component, &self.linker)?;

        let req = exports::weaver::plugin::ensures::EnsureRequest {
            app_path: app_path.to_string(),
            dry_run,
            config: config_json.to_string(),
        };

        bindings
            .weaver_plugin_ensures()
            .call_plan(&mut store, &req)?
            .map_err(|e| {
                anyhow::Error::new(super::PluginError::Other(anyhow::anyhow!(
                    "Plugin plan error: {:?}",
                    e
                )))
            })
    }

    pub fn execute(
        &self,
        app_path: &str,
        dry_run: bool,
        config_json: &str,
    ) -> anyhow::Result<String> {
        let mut store = Store::new(&self.engine, EnsureHost::new());
        let bindings = EnsureProvider::instantiate(&mut store, &self.component, &self.linker)?;

        let req = exports::weaver::plugin::ensures::EnsureRequest {
            app_path: app_path.to_string(),
            dry_run,
            config: config_json.to_string(),
        };

        bindings
            .weaver_plugin_ensures()
            .call_execute(&mut store, &req)?
            .map_err(|e| {
                anyhow::Error::new(super::PluginError::Other(anyhow::anyhow!(
                    "Plugin execute error: {:?}",
                    e
                )))
            })
    }
}
