mod cli;
mod device;
mod gadget;
mod proxy;
mod scripting;
mod setup;
mod bindings;
mod logging;
mod virtual_device;

use clap::Parser;
use crate::device::{CompoundHIDevice, HIDevice};
use std::collections::HashMap;
use std::fs::{OpenOptions};
use std::io::{self, Write};
use std::time::Duration;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use futures::future::join_all;
use rusb::{Context, DeviceHandle, UsbContext};
use tokio::time;
use tracing::{info, error, warn, debug};
use crate::proxy::GlobalState;
use crate::scripting::ScriptContext;

const TICK_RATE_HZ: u64 = 100;

#[tokio::main]
async fn main() {
    if let Err(e) = run_proxy().await {
        setup::toggle_terminal_echo(true);
        error!("error: {}", e);
        std::process::exit(1);
    }
}

async fn run_proxy() -> Result<(), Box<dyn std::error::Error>> {
    logging::init()?;
    debug!("initializing");

    setup::toggle_terminal_echo(false);
    setup::check_root()?;
    setup::check_config_txt()?;
    setup::check_kernel_setup()?;

    let args = cli::Args::parse();
    let script_path = resolve_script_path(args.script.as_deref())?;

    let gadget_created = Arc::new(AtomicBool::new(false));
    let gadget_created_clone = gadget_created.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        setup::toggle_terminal_echo(true);
        println!("CTRL+C detected, cleaning up");
        if gadget_created_clone.load(Ordering::SeqCst) {
            gadget::cleanup_gadget_on_exit();
        }
        std::process::exit(0);
    });

    if let Some(ref path) = script_path {
        info!("using script '{}'", path.display());
    } else {
        info!("no active script");
    }

    if let Some(ref target) = args.target {
        info!("auto-targeting device matching '{}'", target);
    }

    info!("starting usb human interface device proxy");

    loop {
        let device = match select_device(args.target.as_deref()).await {
            Ok(Some(device)) => device,
            Ok(None) => {
                info!("no device selected, rescanning");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            Err(e) => {
                error!("failed to select a device: {}", e);
                if args.target.is_some() {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        info!("{}", device);
        debug!(device = ?device, "selected device for proxying");

        let handle = match setup_device_connection(device.interfaces.first().unwrap()) {
            Ok(h) => h,
            Err(e) => {
                error!("failed to connect to device: {}", e);
                continue;
            }
        };

        // --- Scripting and Virtual Device Setup ---

        let virtual_device_requests = Arc::new(Mutex::new(Vec::new()));

        let global_state = Arc::new(GlobalState {
            gadget_writers: Mutex::new(HashMap::new()),
            virtual_device_requests: virtual_device_requests.clone(),
            target_info: device.interfaces.first().unwrap().clone(),
            num_physical_interfaces: device.interfaces.len(),
            handle_output: handle.clone(),
        });

        let script_context = scripting::load_script_engine(script_path.clone(), global_state.clone());
        let script_context_arc = Arc::new(script_context);

        if let Some((engine, ast, scope_mutex)) = script_context_arc.as_ref() {
            let mut scope = scope_mutex.lock().unwrap();
            let result: Result<(), _> = engine.call_fn(&mut *scope, &ast, "init", ());
            if let Err(e) = result {
                if !e.to_string().contains("Function not found") {
                    warn!("error while executing rhai init() hook: {e}");
                }
            }
        }

        let virtual_devices_to_create = virtual_device_requests.lock().unwrap().clone();
        info!("requested {} virtual devices: {:?}", virtual_devices_to_create.len(), virtual_devices_to_create);

        // --- Gadget Creation ---

        let device_paths = match gadget::create_gadget(&device, &virtual_devices_to_create) {
            Ok(paths) => paths,
            Err(e) => {
                error!("failed to create USB gadget: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        gadget_created.store(true, Ordering::SeqCst);

        gadget::wait_for_host_connection();

        // --- Final State and Proxy Loop ---

        let mut gadget_writers = HashMap::new();
        for (i, path) in device_paths.iter().enumerate() {
            debug!(path = path, "opening gadget for writing");
            let writer = OpenOptions::new().write(true).open(path)
                .map_err(|e| format!("failed to open {} for writing: {}", path, e))?;
            gadget_writers.insert(i, writer);
        }

        *global_state.gadget_writers.lock().unwrap() = gadget_writers;

        // --- Start Script Tick Loop ---
        let script_tick_context = script_context_arc.clone();
        tokio::spawn(async move {
            if script_tick_context.is_some() {
                info!("starting script tick loop at {} Hz", TICK_RATE_HZ);
                let mut interval = time::interval(Duration::from_millis(1000 / TICK_RATE_HZ));
                loop {
                    interval.tick().await;
                    scripting::tick(&script_tick_context);
                }
            }
        });

        info!("beginning proxy loop for {} physical interfaces", device.interfaces.len());
        let mut proxy_tasks = Vec::new();

        for (i, interface) in device.interfaces.iter().enumerate() {
            let interface_clone = interface.clone();
            let script_context_clone = script_context_arc.clone();
            let global_state_clone = global_state.clone();
            let task = tokio::task::spawn_blocking(move || {
                proxy::proxy_loop(interface_clone, script_context_clone, i, global_state_clone)
            });
            proxy_tasks.push(task);
        }

        let results = join_all(proxy_tasks).await;
        for result in results {
            match result {
                Ok(Err(e)) => warn!("proxy loop ended: {}", e),
                Err(e) => error!("proxy task failed to execute: {}", e),
                _ => {}
            }
        }

        warn!("device removed or host disconnected");
        info!("cleaning up");
        let _ = gadget::teardown_gadget("/sys/kernel/config/usb_gadget/hid_proxy");
        gadget_created.store(false, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn setup_device_connection(target_info: &HIDevice) -> Result<Arc<DeviceHandle<Context>>, Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let device = context.devices()?.iter()
        .find(|d| d.bus_number() == target_info.bus && d.address() == target_info.address)
        .ok_or("target device vanished before proxy loop")?;

    info!("proxy loop opening device...");
    let handle = device.open()?;
    debug!("device opened successfully");

    if handle.kernel_driver_active(target_info.interface_num).unwrap_or(false) {
        debug!(iface = target_info.interface_num, "detaching kernel driver");
        handle.detach_kernel_driver(target_info.interface_num)?;
    }
    handle.claim_interface(target_info.interface_num)?;
    debug!(iface = target_info.interface_num, "claimed interface");

    Ok(Arc::new(handle))
}

fn resolve_script_path(script_name: Option<&str>) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if let Some(name) = script_name {
        debug!(script_name = %name, "resolving script path");
        let script_path = setup::resolve_script_path(name);
        if script_path.is_none() {
            return Err(format!("script file '{}' not found", name).into());
        }
        Ok(script_path)
    } else {
        Ok(None)
    }
}

async fn select_device(target: Option<&str>) -> Result<Option<CompoundHIDevice>, Box<dyn std::error::Error>> {
    loop {
        info!("scanning for devices");
        let candidates = device::get_connected_devices();
        debug!(count = candidates.len(), "found candidate devices");

        if candidates.is_empty() {
            info!("awaiting hotplug");
            device::block_till_hotplug().await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        if let Some(target_str) = target {
            if let Some(device) = candidates.iter().find(|d| d.matches(target_str)) {
                debug!(device = ?device, "target device found");
                return Ok(Some(device.clone()));
            } else {
                return Err(format!("target device '{}' not found", target_str).into());
            }
        }

        if candidates.len() == 1 {
            debug!("only one candidate, selecting automatically");
            return Ok(Some(candidates[0].clone()));
        }

        if let Some(selected) = select_device_interactive(&candidates) {
            debug!(device = ?selected, "user selected device");
            return Ok(Some(selected));
        }

        warn!("invalid selection, rescanning...");
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}

fn select_device_interactive(candidates: &[CompoundHIDevice]) -> Option<CompoundHIDevice> {
    println!("found {} devices. Please select one:", candidates.len());
    println!(
        "{:<5} | {:<13} | {:<10} | {:<10} | {}",
        "IDX", "ID", "BUS:ADDR", "INTERFACES", "PRODUCT"
    );
    println!("{:-<5}-+-{:-<13}-+-{:-<10}-+-{:-<12}-+-{:-<20}", "", "", "", "", "");

    for (index, dev) in candidates.iter().enumerate() {
        let id_display = format!("{:04x}:{:04x}", dev.vendor_id, dev.product_id);
        println!(
            "{:<5} | {:<13} | {:03}:{:03}    | {:<10} | {}",
            index,
            id_display,
            dev.bus,
            dev.address,
            dev.interfaces.len(),
            dev.product.as_deref().unwrap_or("Unknown")
        );
    }

    print!("\n> select a device index [e.g., 0]: ");
    io::stdout().flush().unwrap();

    setup::toggle_terminal_echo(true);

    let mut input = String::new();
    let selection = if io::stdin().read_line(&mut input).is_ok() {
        debug!(input = %input.trim(), "user input received");
        input
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|idx| candidates.get(idx).cloned())
    } else {
        None
    };

    setup::toggle_terminal_echo(false);

    selection
}
