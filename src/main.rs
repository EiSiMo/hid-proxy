mod cli;
mod device;
mod gadget;
mod proxy;
mod scripting;
mod setup;
mod bindings;
mod logging;

use clap::Parser;
use crate::device::CompoundHIDevice;
use std::io::{self, Write};
use std::time::Duration;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, error, warn, debug};
use futures::future::join_all;

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
        println!("\nCTRL+C detected, cleaning up");
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

        if let Err(e) = gadget::create_gadget(&device) {
            error!("failed to create USB gadget: {}", e);
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }
        gadget_created.store(true, Ordering::SeqCst);

        gadget::wait_for_host_connection();
        info!("beginning proxy loop for {} interfaces", device.interfaces.len());

        let mut proxy_tasks = Vec::new();
        for (i, interface) in device.interfaces.iter().enumerate() {
            let script_path_clone = script_path.clone();
            let interface_clone = interface.clone();
            let task = tokio::task::spawn_blocking(move || {
                proxy::proxy_loop(interface_clone, script_path_clone, i)
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
