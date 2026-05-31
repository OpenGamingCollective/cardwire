//! Cardwire profiler, only in analysis mode rn
mod dynamic_analysis;
mod helper;
mod models;
mod static_analysis;

pub use dynamic_analysis::check_electron;
pub use models::CardwireProfiler;
