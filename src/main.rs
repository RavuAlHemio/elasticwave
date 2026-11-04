mod config;
mod elastic;
mod token_auth_client;
mod wave_suite;


use std::path::PathBuf;

use clap::Parser;
use tokio::sync::RwLock;

use crate::config::{CONFIG, Config};


#[derive(Parser)]
struct Opts {
    #[arg(short, long, default_value = "config.toml")]
    pub config_path: PathBuf,
}


async fn run() {
    let opts = Opts::parse();

    let config_string = std::fs::read_to_string(&opts.config_path)
        .expect("failed to read config file");
    let config: Config = toml::from_str(&config_string)
        .expect("failed to parse config file");

    CONFIG
        .set(RwLock::new(config))
        .expect("CONFIG already set?!");


}

#[tokio::main]
async fn main() {
    run().await
}
