use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct CardwireArgs {
    #[arg(short, long, num_args = 0..=1, default_missing_value = "true")]
    pub background: Option<bool>,
}
