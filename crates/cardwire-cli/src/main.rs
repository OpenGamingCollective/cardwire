mod args;
mod dbus;
mod display;
use args::{Args, CliMode, Commands, ConfigAction, DebugAction, ManagerAction};
use clap::{CommandFactory, Parser};
use dbus::DaemonClient;

const BIN_NAME: &str = "cardwire";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // Handle completion before connecting to dbus
    if let Commands::Completion { shell } = args.command {
        let mut cmd = Args::command();
        clap_complete::generate(shell, &mut cmd, BIN_NAME, &mut std::io::stdout());
        return Ok(());
    }
    // Now connect
    let connection: zbus::Connection = zbus::connection::Builder::system()?.build().await?;
    let client: DaemonClient<'_> = DaemonClient::connect(&connection).await?;
    match args.command {
        Commands::Set { mode } => {
            let mode_u32 = match mode {
                CliMode::Integrated => 0,
                CliMode::Hybrid => 1,
                CliMode::Manual => 2,
            };

            match client.set_mode(&mode_u32).await {
                Ok(_) => println!("Mode has been set to {}", mode),
                Err(e) => handle_error(e.into()),
            };
        }
        Commands::Get => {
            match client.get_mode().await {
                Ok(response) => {
                    let response: CliMode = match response {
                        0 => CliMode::Integrated,
                        1 => CliMode::Hybrid,
                        2 => CliMode::Manual,
                        // shouldn't happen
                        _ => CliMode::Manual,
                    };
                    println!("Current Mode: {}", response)
                }
                Err(e) => handle_error(e),
            };
        }
        Commands::List { full, json } => {
            if full {
                println!("Full PCI list is not supported in the new D-Bus API");
            } else {
                let mut map = std::collections::BTreeMap::new();
                let objects = client.get_managed_objects().await.unwrap_or_default();
                for (path, interfaces) in objects {
                    let path_str = path.as_str();
                    if let Some(id_str) =
                        path_str.strip_prefix("/com/github/opengamingcollective/cardwire/Gpu/")
                        && let Ok(id) = id_str.parse::<u32>()
                    {
                        let mut blocked = false;
                        for (iface, props) in interfaces {
                            if iface.as_str() == "com.github.opengamingcollective.cardwire.Gpu"
                                && let Some(block_val) = props.get("Block")
                            {
                                blocked = block_val.downcast_ref::<bool>().unwrap_or(false);
                            }
                        }
                        if let Ok(dbus_dev) = client.get_device(id).await {
                            let dev = display::GpuDevice {
                                id,
                                name: dbus_dev.name,
                                pci: dbus_dev.pci,
                                render: dbus_dev.render,
                                card: dbus_dev.card,
                                default: dbus_dev.default,
                                blocked,
                                nvidia: dbus_dev.nvidia,
                                nvidia_minor: dbus_dev.nvidia_minor,
                            };
                            map.insert(id as usize, dev);
                        }
                    }
                }
                if let Err(e) = display::print_devices(map, json) {
                    handle_error(zbus::Error::FDO(Box::new(zbus::fdo::Error::Failed(
                        e.to_string(),
                    ))));
                }
            }
        }
        Commands::Gpu { id, action } => {
            if action.block {
                match client.set_gpu_block(id, true).await {
                    Ok(_) => println!("GPU {} has been blocked", id),
                    Err(e) => handle_error(e.into()),
                };
            } else if action.unblock {
                match client.set_gpu_block(id, false).await {
                    Ok(_) => println!("GPU {} has been unblocked", id),
                    Err(e) => handle_error(e.into()),
                };
            } else if action.lsof {
                match client.lsof(id).await {
                    Ok(map) => {
                        for (path, procs) in map {
                            println!("  {}: {:?}", path, procs);
                        }
                    }
                    Err(e) => handle_error(e),
                };
            } else if action.power {
                match client.get_power_state(id).await {
                    Ok(power_state) => {
                        println!("{}", power_state);
                    }
                    Err(e) => handle_error(e),
                };
            }
        }
        Commands::Config { action } => match action {
            ConfigAction::AutoApplyGpuState { set } => {
                if let Some(val) = set {
                    if let Err(e) = client.set_auto_apply_gpu_state(val).await {
                        handle_error(e.into());
                    } else {
                        println!("AutoApplyGpuState set to {}", val);
                    }
                } else {
                    match client.get_auto_apply_gpu_state().await {
                        Ok(val) => println!("AutoApplyGpuState: {}", val),
                        Err(e) => handle_error(e),
                    }
                }
            }
            ConfigAction::ExperimentalNvidiaBlock { set } => {
                if let Some(val) = set {
                    if let Err(e) = client.set_experimental_nvidia_block(val).await {
                        handle_error(e.into());
                    } else {
                        println!("ExperimentalNvidiaBlock set to {}", val);
                    }
                } else {
                    match client.get_experimental_nvidia_block().await {
                        Ok(val) => println!("ExperimentalNvidiaBlock: {}", val),
                        Err(e) => handle_error(e),
                    }
                }
            }
            ConfigAction::BatteryAutoSwitch { set } => {
                if let Some(val) = set {
                    if let Err(e) = client.set_battery_auto_switch(val).await {
                        handle_error(e.into());
                    } else {
                        println!("BatteryAutoSwitch set to {}", val);
                    }
                } else {
                    match client.get_battery_auto_switch().await {
                        Ok(val) => println!("BatteryAutoSwitch: {}", val),
                        Err(e) => handle_error(e),
                    }
                }
            }
            ConfigAction::Save => {
                if let Err(e) = client.save_to_file().await {
                    handle_error(e);
                } else {
                    println!("Configuration saved");
                }
            }
        },
        Commands::Manager { action } => match action {
            ManagerAction::Status => {
                if let Err(e) = client.manager_status().await {
                    handle_error(e);
                } else {
                    println!("Daemon is alive");
                }
            }
            ManagerAction::RefreshGpu => {
                if let Err(e) = client.refresh_gpu().await {
                    handle_error(e);
                } else {
                    println!("GPU list refreshed");
                }
            }
        },
        Commands::Debug { action } => match action {
            DebugAction::DiagnosticGpu => {
                if let Err(e) = client.diagnostic_gpu().await {
                    handle_error(e);
                } else {
                    // TODO: implement debug
                    println!("DiagnosticGpu signal emitted");
                }
            }
        },
        _ => {}
    }

    Ok(())
}
fn handle_error(err: zbus::Error) {
    match err {
        zbus::Error::MethodError(name, description, _) => {
            if let Some(msg) = description {
                eprintln!("{}", msg);
            } else {
                eprintln!("{}", name);
            }
        }
        zbus::Error::FDO(fdo_err) => match &*fdo_err {
            zbus::fdo::Error::AccessDenied(msg)
            | zbus::fdo::Error::Failed(msg)
            | zbus::fdo::Error::InvalidArgs(msg)
            | zbus::fdo::Error::NotSupported(msg) => eprintln!("{}", msg),
            zbus::fdo::Error::ServiceUnknown(_) => {
                eprintln!("error: cardwired daemon is not running. Is the service up?");
            }
            _ => eprintln!("{}", fdo_err),
        },
        _ => eprintln!("{}", err),
    }
}
