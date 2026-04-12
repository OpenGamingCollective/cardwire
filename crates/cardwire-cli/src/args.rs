use clap::{ArgAction, Args as ClapArgs, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
#[derive(Clone, Debug, ValueEnum)]
pub enum CliMode {
    Integrated,
    Hybrid,
    Manual,
}

#[derive(Parser)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(arg_required_else_help = true, about = "Set to the desired mode")]
    Set {
        #[arg(help("Set to the desired mode"))]
        mode: CliMode,
    },

    #[command(about = "Get the current mode")]
    Get,

    #[command(about = "Print the gpu list")]
    List {
        #[arg(long, help("Print the whole gpu list"), action(ArgAction::SetTrue))]
        full: bool,
        #[arg(
            long,
            help("Print the gpu list in json format"),
            action(ArgAction::SetTrue)
        )]
        json: bool,
    },

    #[command(
        arg_required_else_help = true,
        about = "Manage a specific GPU by its id"
    )]
    Gpu {
        id: u32,
        #[command(flatten)]
        action: GpuAction,
    },
    #[command(about = "Generate shell completions", hide = true)]
    Completion {
        #[arg(help = "The shell to generate the completions for")]
        shell: Shell,
    },
}
#[derive(ClapArgs, Debug)]
#[group(required = true, multiple = false)]
pub struct GpuAction {
    #[arg(long, help = "Block a specific gpu")]
    pub block: bool,

    #[arg(long, help = "Unblock a specific gpu")]
    pub unblock: bool,
}
