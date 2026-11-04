use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use url::Url;


pub(crate) static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();


#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Config {
    pub wave_suite: WaveSuiteConfig,
    pub elastic: ElasticConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WaveSuiteConfig {
    pub base_url: Url,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ElasticConfig {
    pub base_url: Url,
    pub username: String,
    pub password: String,
    pub active_alarms_index: String,
    pub alarm_info_index: String,
}
