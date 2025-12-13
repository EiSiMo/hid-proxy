use rhai::{AST, Engine, Scope};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::bindings::{self, Interface};
use crate::proxy::GlobalState;
use tracing::{info, error, debug};

pub type ScriptContext = Option<(Engine, AST, Mutex<Scope<'static>>)>;

pub fn load_script_engine(script_path: Option<PathBuf>, shared_state: Arc<GlobalState>) -> ScriptContext {
    if let Some(path) = script_path {
        info!("loading script {}", path.display());
        debug!(path = %path.display(), "compiling script");
        let mut engine = Engine::new();

        bindings::register_native_fns(&mut engine, Arc::clone(&shared_state));

        match engine.compile_file(path) {
            Ok(ast) => {
                debug!("script compiled successfully, creating scope");
                let mut scope = Scope::new();
                scope.push("global_state", Arc::clone(&shared_state));
                scope.push("device", shared_state.target_info.clone());

                // Run the script once to initialize global variables (e.g. 'let VIRTUAL_MOUSE = ...')
                debug!("initializing script globals");
                if let Err(e) = engine.run_ast_with_scope(&mut scope, &ast) {
                    error!("error initializing script globals: {}", e);
                    std::process::exit(1);
                }

                info!("script compiled and scope created.");
                return Some((engine, ast, Mutex::new(scope)));
            }
            Err(e) => {
                error!("script compilation error: {}", e);
                std::process::exit(1);
            }
        }
    }
    debug!("no script path provided, scripting disabled");
    None
}

pub fn process_payload(
    engine_opt: &Arc<ScriptContext>,
    interface: Interface,
    direction: &str,
    data: &[u8],
) {
    if let Some((engine, ast, scope_mutex)) = engine_opt.as_ref() {
        debug!(?direction, len = data.len(), ?data, "calling 'process' hook in script");
        let blob: Vec<rhai::Dynamic> = data.iter().map(|&b| (b as i64).into()).collect();

        if let Ok(mut scope) = scope_mutex.lock() {
            let result: Result<(), _> =
                engine.call_fn(&mut *scope, ast, "process", (interface, direction.to_string(), blob));

            if let Err(e) = result {
                if !e.to_string().contains("Function not found") {
                    error!("error while executing rhai process() hook: {e}");
                }
            }
        }
    }
}

pub fn tick(engine_opt: &Arc<ScriptContext>) {
    if let Some((engine, ast, scope_mutex)) = engine_opt.as_ref() {
        if let Ok(mut scope) = scope_mutex.lock() {
            let result: Result<(), _> = engine.call_fn(&mut *scope, ast, "tick", ());
            if let Err(e) = result {
                if !e.to_string().contains("Function not found") {
                    error!("error while executing rhai tick() hook: {e}");
                }
            }
        }
    }
}
