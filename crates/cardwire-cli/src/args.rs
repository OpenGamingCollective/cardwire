use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize, zbus::zvariant::Type,
)]
#[serde(rename_all = "snake_case")]
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

    #[command(about = "Launch a program on the specified GPU")]
    Launch {
        #[arg(long, help = "Select the gpu")]
        gpu: Option<u32>,

        #[arg(
            required = true,
            trailing_var_arg = true,
            allow_hyphen_values = true,
            help = "The program to launch and its arguments (e.g., `nvtop -s`)"
        )]
        program: Vec<String>,
    },

    #[command(about = "Generate shell completions", hide = true)]
    Completion {
        #[arg(help = "The shell to generate the completions for")]
        shell: Shell,
    },
    #[command(about = "Complete gpus for shell", hide = true)]
    CompleteGpus,
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
    #[command(about = "Get or set BatteryAutoSwitchMode")]
    BatteryAutoSwitchMode {
        #[arg(help = "Value to set")]
        set: Option<CliMode>,
    },
    #[command(about = "Save current configuration to file")]
    Save,
}

#[derive(Subcommand, Debug)]
pub enum ManagerAction {
    #[command(about = "Check if daemon is alive")]
    Status,
}

#[derive(Subcommand, Debug)]
pub enum DebugAction {
    #[command(about = "Run GPU diagnostics")]
    DiagnosticGpu,
    #[command(about = "Refresh GPU list in daemon")]
    RefreshGpu,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_mode_display_all_variants() {
        assert_eq!(CliMode::Integrated.to_string(), "Integrated");
        assert_eq!(CliMode::Hybrid.to_string(), "Hybrid");
        assert_eq!(CliMode::Manual.to_string(), "Manual");
        assert_eq!(CliMode::Smart.to_string(), "Smart");
    }

    #[test]
    fn test_args_parse_set_command() {
        let args = Args::try_parse_from(["cardwire", "set", "hybrid"]).unwrap();
        match args.command {
            Commands::Set { mode } => assert!(matches!(mode, CliMode::Hybrid)),
            _ => panic!("expected Set command"),
        }
    }

    #[test]
    fn test_args_parse_get_command() {
        let args = Args::try_parse_from(["cardwire", "get"]).unwrap();
        assert!(matches!(args.command, Commands::Get));
    }

    #[test]
    fn test_args_parse_list_command() {
        let args = Args::try_parse_from(["cardwire", "list"]).unwrap();
        match args.command {
            Commands::List { full, json } => {
                assert!(!full);
                assert!(!json);
            }
            _ => panic!("expected List command"),
        }
    }

    #[test]
    fn test_args_parse_list_with_json_flag() {
        let args = Args::try_parse_from(["cardwire", "list", "--json"]).unwrap();
        match args.command {
            Commands::List { json, .. } => assert!(json),
            _ => panic!("expected List command"),
        }
    }

    #[test]
    fn test_args_parse_list_with_full_flag() {
        let args = Args::try_parse_from(["cardwire", "list", "--full"]).unwrap();
        match args.command {
            Commands::List { full, .. } => assert!(full),
            _ => panic!("expected List command"),
        }
    }

    #[test]
    fn test_args_parse_gpu_block_command() {
        let args = Args::try_parse_from(["cardwire", "gpu", "1", "--block"]).unwrap();
        match args.command {
            Commands::Gpu { id, action } => {
                assert_eq!(id, 1);
                assert!(action.block);
                assert!(!action.unblock);
            }
            _ => panic!("expected Gpu command"),
        }
    }

    #[test]
    fn test_args_parse_set_all_modes() {
        for mode_str in ["integrated", "hybrid", "manual", "smart"] {
            let result = Args::try_parse_from(["cardwire", "set", mode_str]);
            assert!(result.is_ok(), "failed to parse mode: {mode_str}");
        }
    }

    #[test]
    fn test_args_parse_set_invalid_mode_fails() {
        let result = Args::try_parse_from(["cardwire", "set", "asusmux"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_parse_config_auto_apply() {
        let args =
            Args::try_parse_from(["cardwire", "config", "auto-apply-gpu-state", "true"]).unwrap();
        match args.command {
            Commands::Config { action } => match action {
                ConfigAction::AutoApplyGpuState { set } => assert_eq!(set, Some(true)),
                _ => panic!("expected AutoApplyGpuState"),
            },
            _ => panic!("expected Config command"),
        }
    }

    #[test]
    fn test_args_parse_manager_status() {
        let args = Args::try_parse_from(["cardwire", "manager", "status"]).unwrap();
        assert!(matches!(
            args.command,
            Commands::Manager {
                action: ManagerAction::Status
            }
        ));
    }

    #[test]
    fn test_args_parse_debug_diagnostic() {
        let args = Args::try_parse_from(["cardwire", "debug", "diagnostic-gpu"]).unwrap();
        assert!(matches!(
            args.command,
            Commands::Debug {
                action: DebugAction::DiagnosticGpu
            }
        ));
    }
}
