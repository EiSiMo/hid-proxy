use rhai::{AST, Engine, Scope};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::bindings::register_native_fns;
use crate::proxy::SharedState;
use tracing::{info, warn, debug};

pub fn load_script_engine(script_path: Option<PathBuf>, shared_state: Arc<SharedState>) -> Option<(Engine, AST, Mutex<Scope<'static>>)> {
    if let Some(path) = script_path {
        info!("loading script {}", path.display());
        debug!(path = %path.display(), "compiling script");
        let mut engine = Engine::new();

        register_native_fns(&mut engine, Arc::clone(&shared_state));

        match engine.compile_file(path) {
            Ok(ast) => {
                debug!("script compiled successfully, creating scope");
                let mut scope = Scope::new();
                scope.push("device", shared_state.target_info.clone()); // Add device info to scope
                if let Err(e) = engine.run_ast_with_scope(&mut scope, &ast) {
                    warn!("script initialization error: {}", e);
                    return None;
                }
                info!("script compiled and initialized successfully.");
                debug!("Rhai engine and scope initialized");
                return Some((engine, ast, Mutex::new(scope)));
            }
            Err(e) => {
                warn!("script compilation error: {}", e);
                return None;
            }
        }
    }
    debug!("no script path provided, scripting disabled");
    None
}

pub fn process_payload(
    engine_opt: &Option<(Engine, AST, Mutex<Scope<'static>>)>,
    direction: &str,
    data: &[u8],
) {
    if let Some((engine, ast, scope_mutex)) = engine_opt {
        debug!(?direction, len = data.len(), ?data, "calling 'process' hook in script");
        let blob: Vec<rhai::Dynamic> = data.iter().map(|&b| (b as i64).into()).collect();

        if let Ok(mut scope) = scope_mutex.lock() {
            let result: Result<(), _> =
                engine.call_fn(&mut *scope, ast, "process", (direction.to_string(), blob));

            if let Err(e) = result {
                warn!("error while executing rhai script: {e}");
            }
        }
    }
    // Note: The script is responsible for forwarding the data.
    // This function does not return the (potentially modified) payload.
}
