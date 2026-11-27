use std::sync::OnceLock;

use reqwest::Certificate;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use url::Url;


pub(crate) static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();


#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Config {
    pub wave_suite: WaveSuiteConfig,
    pub elastic: ElasticConfig,
    pub elastic_wave: ElasticWaveConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WaveSuiteConfig {
    pub base_url: Url,
    #[serde(default)] pub custom_auth_url: Option<Url>,
    #[serde(default)] pub custom_logout_url: Option<Url>,
    #[serde(default)] pub custom_fault_management_base_url: Option<Url>,
    pub username: String,
    pub password: String,
    #[serde(default)] pub custom_trusted_certificate_file: Option<String>,

    #[serde(default = "WaveSuiteConfig::default_trust_system_certificate_authorities")]
    pub trust_system_certificate_authorities: bool,
}
impl WaveSuiteConfig {
    fn default_trust_system_certificate_authorities() -> bool { true }

    pub fn auth_url(&self) -> Url {
        if let Some(cau) = self.custom_auth_url.as_ref() {
            cau.clone()
        } else {
            self.base_url.join("rest-gateway/rest/api/v1/auth/token")
                .expect("failed to construct auth URL from base URL and path")
        }
    }

    pub fn logout_url(&self) -> Url {
        if let Some(clu) = self.custom_logout_url.as_ref() {
            clu.clone()
        } else {
            self.base_url.join("rest-gateway/rest/api/v1/auth/revocation")
                .expect("failed to construct logout URL from base URL and path")
        }
    }

    pub fn fault_management_base_url(&self) -> Url {
        if let Some(cfmbu) = self.custom_fault_management_base_url.as_ref() {
            cfmbu.clone()
        } else {
            self.base_url.join("FaultManagement/rest/api/")
                .expect("failed to construct fault management base URL from WaveSuite base URL and path")
        }
    }

    pub fn custom_trusted_certificates(&self) -> Result<Option<Vec<Certificate>>, reqwest::Error> {
        if let Some(ctcf) = self.custom_trusted_certificate_file.as_ref() {
            let bundle_bytes = std::fs::read(ctcf)
                .expect("failed to read trusted certificate bundle");
            Certificate::from_pem_bundle(&bundle_bytes)
                .map(|certs| Some(certs))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ElasticConfig {
    pub base_url: Url,
    pub username: String,
    pub password: String,
    pub active_alarms_index: String,
    pub alarm_info_index: String,
    #[serde(default)] pub custom_trusted_certificate_file: Option<String>,

    #[serde(default = "ElasticConfig::default_trust_system_certificate_authorities")]
    pub trust_system_certificate_authorities: bool,
}
impl ElasticConfig {
    fn default_trust_system_certificate_authorities() -> bool { true }

    pub fn custom_trusted_certificates(&self) -> Result<Option<Vec<Certificate>>, reqwest::Error> {
        if let Some(ctcf) = self.custom_trusted_certificate_file.as_ref() {
            let bundle_bytes = std::fs::read(ctcf)
                .expect("failed to read trusted certificate bundle");
            Certificate::from_pem_bundle(&bundle_bytes)
                .map(|certs| Some(certs))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ElasticWaveConfig {
    pub query_interval_s: u64,
}
