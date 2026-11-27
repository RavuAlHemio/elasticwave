mod config;
mod elastic;
mod token_auth_client;
mod wave_suite;


use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::config::{CONFIG, Config};
use crate::elastic::{AuthInfo, ElasticClient};
use crate::token_auth_client::TokenAuthClient;
use crate::wave_suite::WaveSuiteClient;


#[derive(Parser)]
struct Opts {
    #[arg(short, long, default_value = "config.toml")]
    pub config_path: PathBuf,

    #[arg(short, long)]
    pub token_pair_file: Option<PathBuf>,
}


async fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let opts = Opts::parse();

    let config_string = std::fs::read_to_string(&opts.config_path)
        .expect("failed to read config file");
    let config: Config = toml::from_str(&config_string)
        .expect("failed to parse config file");

    // set up WaveSuite client
    let ws_custom_certs = config.wave_suite.custom_trusted_certificates()
        .expect("failed to read file of WaveSuite custom trusted certificates");
    let ws_auth_client = TokenAuthClient::new(
        config.wave_suite.auth_url(),
        config.wave_suite.logout_url(),
        config.wave_suite.username.clone(),
        config.elastic.password.clone(),
        ws_custom_certs.as_ref().map(|cc| cc.as_slice()),
        config.wave_suite.trust_system_certificate_authorities,
    );
    let wave_suite_client = WaveSuiteClient::new(ws_auth_client, config.wave_suite.fault_management_base_url());

    // set up Elastic client
    let el_custom_certs = config.elastic.custom_trusted_certificates()
        .expect("failed to read file of custom trusted certificates");
    let el_client = ElasticClient::new(
        config.elastic.base_url.clone(),
        AuthInfo::Basic {
            username: config.elastic.username.clone(),
            password: config.elastic.password.clone(),
        },
        el_custom_certs.as_ref().map(|cc| cc.as_slice()),
        config.elastic.trust_system_certificate_authorities,
    );

    let sleep_interval_s = config.elastic_wave.query_interval_s;

    CONFIG
        .set(RwLock::new(config))
        .expect("CONFIG already set?!");

    loop {
        let alarm_ids = wave_suite_client.get_live_alarm_ids(Some(0))
            .await.expect("failed to get alarms");

        if let Some(tpf) = opts.token_pair_file.as_ref() {
            // store token pair
            if let Some(token_pair) = wave_suite_client.token_pair().await {
                let token_pair_string = serde_json::to_string_pretty(&token_pair)
                    .expect("failed to serialize token pair");
                std::fs::write(tpf, &token_pair_string)
                    .expect("failed to write out token pair");
            }
        }

        println!("[");
        let mut is_first = true;
        for alarm in &alarm_ids {
            if is_first {
                is_first = false;
            } else {
                println!(",");
            }
            let alarm = wave_suite_client.get_alarm_by_id(alarm)
                .await.expect("failed to get alarm");
            let alarm_json = serde_json::to_string(&alarm.data)
                .expect("failed to serialize alarm to JSON");
            println!("{}", alarm_json);
        }
        println!("]");

        // sleepy eepy
        sleep(Duration::from_secs(sleep_interval_s)).await;
    }
}

#[tokio::main]
async fn main() {
    run().await
}
