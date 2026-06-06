use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::fmt;
#[derive(Clone, Debug, ValueEnum)]
pub enum CliMode {
    Integrated,
    Hybrid,
    Manual,
    Smart,
}
impl fmt::Display for CliMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CliMode::Integrated => write!(f, "Integrated"),
            CliMode::Hybrid => write!(f, "Hybrid"),
            CliMode::Manual => write!(f, "Manual"),
            CliMode::Smart => write!(f, "Smart"),
        }
    }
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
        #[arg(
            long,
            help("Print the whole pci list"),
            action(clap::ArgAction::SetTrue)
        )]
        full: bool,
        #[arg(
            long,
            help("Print the gpu list in json format"),
            action(clap::ArgAction::SetTrue)
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

    #[command(about = "Manage daemon configuration", arg_required_else_help = true)]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    #[command(about = "Manager operations", arg_required_else_help = true)]
    Manager {
        #[command(subcommand)]
        action: ManagerAction,
    },

    #[command(about = "Debug operations", arg_required_else_help = true)]
    Debug {
        #[command(subcommand)]
        action: DebugAction,
    },

    #[command(about = "Generate shell completions", hide = true)]
    Completion {
        #[arg(help = "The shell to generate the completions for")]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    #[command(about = "Get or set AutoApplyGpuState")]
    AutoApplyGpuState {
        #[arg(help = "Value to set")]
        set: Option<bool>,
    },
    #[command(about = "Get or set ExperimentalNvidiaBlock")]
    ExperimentalNvidiaBlock {
        #[arg(help = "Value to set")]
        set: Option<bool>,
    },
    #[command(about = "Get or set BatteryAutoSwitch")]
    BatteryAutoSwitch {
        #[arg(help = "Value to set")]
        set: Option<bool>,
    },
    #[command(about = "Save current configuration to file")]
    Save,
}

#[derive(Subcommand, Debug)]
pub enum ManagerAction {
    #[command(about = "Check if daemon is alive")]
    Status,
    #[command(about = "Refresh GPU list in daemon")]
    RefreshGpu,
}

#[derive(Subcommand, Debug)]
pub enum DebugAction {
    #[command(about = "Run GPU diagnostics")]
    DiagnosticGpu,
}

#[derive(ClapArgs, Debug)]
#[group(required = true, multiple = false)]
pub struct GpuAction {
    #[arg(long, help = "Block a specific gpu")]
    pub block: bool,

    #[arg(long, help = "Unblock a specific gpu")]
    pub unblock: bool,

    #[arg(long, help = "List open files on the GPU")]
    pub lsof: bool,

    #[arg(long, help = "Get GPU power state")]
    pub power: bool,
}
