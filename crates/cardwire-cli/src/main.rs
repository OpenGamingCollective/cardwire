mod args;
mod dbus;
mod output;
use args::{Args, CliMode, Commands};
use clap::{CommandFactory, Parser};
use dbus::DaemonClient;

const BIN_NAME: &str = "cardwire";

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    /*
       Handle completion before connecting to dbus
    */

    if let Commands::Completion { shell } = args.command {
        let mut cmd = Args::command();
        clap_complete::generate(shell, &mut cmd, BIN_NAME, &mut std::io::stdout());
        return Ok(());
    }
    let connection: zbus::Connection = zbus::connection::Builder::system()?.build().await?;
    let client: DaemonClient<'_> = DaemonClient::connect(&connection).await?;

    match args.command {
        Commands::Set { mode } => {
            let mode_string = match mode {
                CliMode::Integrated => "integrated".to_string(),
                CliMode::Hybrid => "hybrid".to_string(),
                CliMode::Manual => "manual".to_string(),
            };

            match client.set_mode(&mode_string).await {
                Ok(_) => println!("Mode has been set to {}", mode_string),
                Err(e) => handle_error(e),
            };
        }
        Commands::Get => {
            match client.get_mode().await {
                Ok(response) => println!("{}", response),
                Err(e) => handle_error(e),
            };
        }
        Commands::List { full: _, json: _ } => match client.list_gpus().await {
            Ok(mut response) => {
                response.sort_by_key(|row| row.0);
                output::print_gpu_table(&response);
            }
            Err(e) => handle_error(e),
        },
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
