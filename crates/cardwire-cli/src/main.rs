mod args;
mod client;
mod output;

use clap::Parser;

use args::{Args, Commands, GpuCommands};
use client::DaemonClient;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let connection = zbus::connection::Builder::system()?.build().await?;
    let client = DaemonClient::connect(&connection).await?;

    match args.command {
        Commands::Set { mode } => {
            //let response = client.set_mode(mode).await;
            match client.set_mode(mode).await{
                Ok(response) => println!("{}", response),
                Err(zbus::Error::MethodError(name, description, _)) => {
                    println!("{}", description.unwrap_or_else(|| name.to_string()))
                }
                Err(e) => println!("Error: {e:?}"),
            };
        }
        Commands::Get => {
            let response_result = client.get_mode().await;
            match response_result{
                Ok(response) => println!("{}", response),
                Err(e) => println!("Error: {e:?}"),
            };
        }
        Commands::List => {
            let mut response = client.list_gpus().await?;
            response.sort_by_key(|row| row.0);
            output::print_gpu_table(&response);
        }
        Commands::Gpu { id, command } => match command {
            GpuCommands::Block { state } => {
                let blocked = match state.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => {
                        return Err(
                            format!("Invalid state '{}'. Expected: on or off", state).into()
                        );
                    }
                };
                let response = client.set_gpu_block(id, blocked).await?;
                println!("{}", response);
            }
            GpuCommands::Info => {
                let response = client.get_gpu_info().await?;
                println!("{}", response);
            }
        }
    }

    Ok(())
}
