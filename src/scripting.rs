use rhai::{AST, Engine, Scope};
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::bindings::register_native_fns;
use crate::proxy::SharedState;

pub fn load_script_engine(script_name: Option<String>, shared_state: Arc<SharedState>) -> Option<(Engine, AST, Mutex<Scope<'static>>)> {
    if let Some(name) = script_name {
        let path_str = format!("examples/{}.rhai", name);
        let path = Path::new(&path_str);

        if path.exists() {
            println!("[*] loading script {}", path_str);
            let mut engine = Engine::new();

            register_native_fns(&mut engine, Arc::clone(&shared_state));

            match engine.compile_file(path_str.into()) {
                Ok(ast) => {
                    // Create a persistent scope
                    let mut scope = Scope::new();

                    // Run the AST once to initialize global variables (like history buffers)
                    if let Err(e) = engine.run_ast_with_scope(&mut scope, &ast) {
                        println!("[!] script initialization error: {}", e);
                        return None;
                    }

                    println!("[*] script compiled and initialized successfully.");
                    return Some((engine, ast, Mutex::new(scope)));
                }
                Err(e) => println!("[!] script compilation error: {}", e),
            }
        } else {
            println!(
                "[!] script file {} not found, running without script",
                path_str
            );
        }
    }
    None
}

pub fn process_payload(
    engine_opt: &Option<(Engine, AST, Mutex<Scope<'static>>)>,
    direction: &str,
    data: &[u8],
) -> Vec<u8> {
    if let Some((engine, ast, scope_mutex)) = engine_opt {
        let blob: Vec<rhai::Dynamic> = data.iter().map(|&b| (b as i64).into()).collect();

        // Lock the scope so we can reuse the state from the previous packet
        if let Ok(mut scope) = scope_mutex.lock() {
            let result: Result<Vec<rhai::Dynamic>, _> =
                engine.call_fn(&mut *scope, ast, "process", (direction.to_string(), blob));

            match result {
                Ok(modified_blob) => {
                    return modified_blob
                        .iter()
                        .map(|d| d.as_int().unwrap_or(0) as u8)
                        .collect();
                }
                Err(e) => {
                    println!("[!] error while executing rhai script: {e}");
                    return data.to_vec();
                }
            }
        }
    }
    data.to_vec()
}