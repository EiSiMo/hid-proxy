use rhai::{AST, Engine, Scope};
use std::path::Path;

pub fn load_script_engine(script_name: Option<String>) -> Option<(Engine, AST)> {
    if let Some(name) = script_name {
        let path_str = format!("scripts/{}.rhai", name);
        let path = Path::new(&path_str);

        if path.exists() {
            println!("[*] loading script {}", path_str);
            let engine = Engine::new();
            match engine.compile_file(path_str.into()) {
                Ok(ast) => {
                    println!("[*] script compiled successfully.");
                    return Some((engine, ast));
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
    engine_opt: &Option<(Engine, AST)>,
    direction: &str,
    data: &[u8],
) -> Vec<u8> {
    if let Some((engine, ast)) = engine_opt {
        let blob: Vec<rhai::Dynamic> = data.iter().map(|&b| (b as i64).into()).collect();
        let mut scope = Scope::new();

        let result: Result<Vec<rhai::Dynamic>, _> =
            engine.call_fn(&mut scope, ast, "process", (direction.to_string(), blob));

        match result {
            Ok(modified_blob) => {
                return modified_blob
                    .iter()
                    .map(|d| d.as_int().unwrap_or(0) as u8)
                    .collect();
            }
            Err(_) => {
                return data.to_vec();
            }
        }
    }
    data.to_vec()
}
