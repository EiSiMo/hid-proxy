use rhai::{AST, Engine, Scope};
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::bindings::register_native_fns;
use crate::proxy::SharedState;

pub fn load_script_engine(script_name: Option<String>, shared_state: Arc<SharedState>) -> Option<(Engine, AST, Mutex<Scope<'static>>)> {
    if let Some(name) = script_name {
        let example_path_str = format!("examples/{}", name);
        let example_path = Path::new(&example_path_str);
        let direct_path = Path::new(&name);

        let chosen_path = if example_path.exists() {
            Some(example_path.to_path_buf())
        } else if direct_path.exists() {
            Some(direct_path.to_path_buf())
        } else {
            None
        };

        if let Some(path) = chosen_path {
            println!("[*] loading script {}", path.display());
            let mut engine = Engine::new();

            register_native_fns(&mut engine, Arc::clone(&shared_state));

            match engine.compile_file(path) {
                Ok(ast) => {
                    let mut scope = Scope::new();
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
            println!("[!] script file '{name}' not found");
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

        if let Ok(mut scope) = scope_mutex.lock() {
            let result: Result<(), _> =
                engine.call_fn(&mut *scope, ast, "process", (direction.to_string(), blob));

            if let Err(e) = result {
                println!("[!] error while executing rhai script: {e}");
            }
        }
    }
    data.to_vec()
}
