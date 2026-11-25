use std::fmt;

use serde::{Deserialize, Serialize};
use tracing::{debug, error};
use url::Url;
use urlencoding;

use crate::token_auth_client::{self, ResponseResult, TokenAuthClient, TokenPair};


#[derive(Debug)]
pub enum Error {
    Url(url::ParseError),
    Authentication(String),
    ErrorStatus {
        status_code: u16,
        body: String,
    },
    HttpClient(token_auth_client::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(e) => write!(f, "URL construction error: {}", e),
            Self::Authentication(s) => write!(f, "authentication error: {}", s),
            Self::ErrorStatus { status_code, body }
                => write!(f, "WaveSuite responded with error: [{}] {}", status_code, body),
            Self::HttpClient(e) => write!(f, "HTTP client error: {}", e),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Url(e) => Some(e),
            Self::HttpClient(e) => Some(e),
            Self::Authentication(_)|Self::ErrorStatus { .. } => None,
        }
    }
}
impl From<url::ParseError> for Error {
    fn from(value: url::ParseError) -> Self {
        Self::Url(value)
    }
}
impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::HttpClient(token_auth_client::Error::Reqwest(value))
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::HttpClient(token_auth_client::Error::JsonParsing(value))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Alarm {
    // TODO
    pub data: AlarmData,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AlarmIdListResponseOuter {
    pub response: AlarmIdListResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AlarmIdListResponse {
    #[serde(rename = "data")] pub alarm_id_list: Vec<String>,
    #[serde(default)] pub errors: Option<Vec<serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AlarmResponseOuter {
    pub response: AlarmResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AlarmResponse {
    pub data: AlarmData,
    #[serde(default)] pub errors: Option<Vec<serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmData {
    pub fdn: String,
    pub object_full_name: String,
    pub source_type: String,
    pub source_system: String,
    pub severity: String,
    pub previous_severity: String,
    pub original_severity: String,
    pub highest_severity: String,
    pub probable_cause: String,
    pub alarm_name: String,
    pub specific_problem: String,
    pub alarm_type: String,
    pub affected_object: String,
    pub affected_object_type: String,
    pub affected_object_name: String,
    pub acknowledged: bool,
    pub was_acknowledged: bool,
    pub acknowledged_by: String,
    pub cleared_by: String,
    pub deleted_by: String,
    pub first_time_detected: i64,
    pub last_time_detected: i64,
    pub last_time_severity_changed: Option<i64>,
    pub last_time_escalated: Option<i64>,
    pub last_time_de_escalated: Option<i64>,
    pub last_time_cleared: Option<i64>,
    pub last_time_acknowledged: Option<i64>,
    pub node_time_offset: i64,
    pub frequency: Option<i64>,
    pub number_of_occurrences: i64,
    pub number_of_occurrences_since_clear: i64,
    pub number_of_occurrences_since_ack: i64,
    pub service_affecting: Option<bool>,
    pub implicitly_cleared: bool,
    pub additional_text: String,
    pub ne_id: String,
    pub ne_name: String,
    pub user_text: String,
    pub admin_state: String,
    pub impact: i64,
    pub root_cause: bool,
}


pub struct WaveSuiteClient {
    client: TokenAuthClient,
    fault_api_root: Url,
}
impl WaveSuiteClient {
    /// Creates a new WaveSuite client.
    ///
    /// `fault_api_root` mostly has the format
    /// `https://wavesuite.example.com/FaultManagement/rest/api/`.
    pub fn new(client: TokenAuthClient, fault_api_root: Url) -> Self {
        Self {
            client,
            fault_api_root,
        }
    }

    pub async fn get_live_alarm_ids(&self, impact_greater_than: Option<u32>) -> Result<Vec<String>, Error> {
        let mut alarms_url = self.fault_api_root.join("v2/alarms")?;
        if let Some(igt) = impact_greater_than {
            alarms_url.query_pairs_mut()
                .append_pair("alarmFilter", &format!("impact>{}", igt));
        };
        let mut alarm_ids = Vec::new();
        loop {
            let this_alarms_url = alarms_url.join(&format!("?pageStart={}", alarm_ids.len())).unwrap();
            let resp_res = self.client.get(&this_alarms_url).await;
            let resp = response_result_to_result(resp_res).await?;
            let resp_string = resp.text().await?;
            debug!("live alarm IDs list response: {:?}", resp_string);
            let resp_obj: AlarmIdListResponseOuter = serde_json::from_str(&resp_string)?;
            if let Some(errors) = resp_obj.response.errors.as_ref() {
                for error in errors {
                    error!("error while getting live alarm IDs: {:?}", error);
                }
            }
            if resp_obj.response.alarm_id_list.len() == 0 {
                break;
            }
            alarm_ids.extend(resp_obj.response.alarm_id_list);
        }
        Ok(alarm_ids)
    }

    pub async fn get_alarm_by_id(&self, alarm_id: &str) -> Result<Alarm, Error> {
        // TODO: filter
        let alarm_id_encoded = urlencoding::encode(alarm_id);
        let alarm_url = self.fault_api_root.join(&format!("v2/alarms/{}", alarm_id_encoded))?;
        let resp_res = self.client.get(&alarm_url).await;
        let resp = response_result_to_result(resp_res).await?;
        let resp_string = resp.text().await?;
            debug!("live alarm response: {:?}", resp_string);
        let resp_obj: AlarmResponseOuter = serde_json::from_str(&resp_string)?;
        Ok(Alarm {
            data: resp_obj.response.data,
        })
        /*
        let resp_obj: AlarmIdListResponseOuter = serde_json::from_str(&resp_string)?;
        if let Some(errors) = resp_obj.response.errors.as_ref() {
            for error in errors {
                error!("error while getting live alarm IDs: {:?}", error);
            }
        }
        if resp_obj.response.alarm_id_list.len() == 0 {
            break;
        }
        alarm_ids.extend(resp_obj.response.alarm_id_list);
        Ok(alarm_ids)
        */
    }

    pub async fn token_pair(&self) -> Option<TokenPair> {
        self.client.get_token_pair().await
    }
}


async fn response_result_to_result(response_result: ResponseResult) -> Result<reqwest::Response, Error> {
    match response_result {
        ResponseResult::SuccessWithBody(r) => {
            Ok(r)
        },
        ResponseResult::CredentialMissing => {
            Err(Error::Authentication(format!("authentication failed")))
        },
        ResponseResult::AuthenticationIssue(response) => {
            let status = response.status();
            let body = response.text().await?;
            Err(Error::Authentication(format!("[{}] {}", status, body)))
        },
        ResponseResult::ErrorStatus(response) => {
            let status = response.status();
            let body = response.text().await?;
            Err(Error::ErrorStatus { status_code: status.as_u16(), body })
        },
        ResponseResult::Error(error) => {
            Err(Error::HttpClient(error))
        },
    }
}
