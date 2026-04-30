mod args;
mod dbus;
mod display;
use args::{Args, CliMode, Commands};
use clap::{CommandFactory, Parser};
use dbus::DaemonClient;

use crate::display::{print_devices, print_devices_pci};

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
                match client.list_devices_pci().await {
                    Ok(response) => {
                        print_devices_pci(response)?;
                    }
                    Err(e) => handle_error(e),
                }
            } else {
                match client.list_devices().await {
                    Ok(response) => {
                        print_devices(response, json)?;
                    }
                    Err(e) => handle_error(e),
                }
            }
        }
        Commands::Gpu { id, action } => {
            match client.set_gpu_block(id, action.block).await {
                Ok(_) => println!("Mode has been set to {} on GPU {}", action.block, id),
                Err(e) => handle_error(e),
            };
        }
        _ => {}
    }

    Ok(())
}
fn handle_error(err: zbus::Error) {
    match err {
        zbus::Error::MethodError(name, description, _) => {
            eprintln!("{}", description.unwrap_or_else(|| name.to_string()))
        }
        zbus::Error::FDO(fdo_err) => match *fdo_err {
            zbus::fdo::Error::ServiceUnknown(content) => {
                eprint!("error: {} \n is the service up?", content)
            }
            other => eprintln!("FDO error: {}", other),
        },
        e => eprintln!("error: {e:?}"),
    }
}
