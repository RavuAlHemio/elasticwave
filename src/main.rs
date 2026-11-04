mod config;
mod elastic;
mod wave_suite;


use std::path::PathBuf;

use clap::Parser;


#[derive(Parser)]
struct Opts {
    #[arg(short, long, default_value = "config.toml")]
    pub config_path: PathBuf,
}


async fn run() {
    let opts = Opts::parse();
}

#[tokio::main]
async fn main() {
    run().await
}
