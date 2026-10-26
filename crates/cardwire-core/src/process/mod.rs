pub mod errors;
#[allow(clippy::module_inception)]
mod process;

pub use process::{
    check_env, get_real_process_name, get_steam_app_id, read_proc_cmdline, read_proc_environ,
    strip_nix_wrap,
};
