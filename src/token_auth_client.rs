use std::fmt;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::sync::Mutex;
use url::Url;


macro_rules! impl_client_state {
    ($has_name:ident, $get_name:ident, $clear_name:ident, $set_name:ident, $prop_name:ident, $type:ty $(,)?) => {
        pub fn $has_name(&self) -> bool { self.$prop_name.is_some() }
        pub fn $get_name(&self) -> Option<$type> { self.$prop_name.clone() }
        pub fn $clear_name(&mut self) { self.$prop_name = None; }
        pub fn $set_name(&mut self, new_value: String) { self.$prop_name = Some(new_value); }
    };
}


struct ClientState {
    access_token: Option<String>,
    refresh_token: Option<String>,
}
impl ClientState {
    pub const fn new() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
        }
    }

    impl_client_state!(
        has_access_token,
        access_token,
        clear_access_token,
        set_access_token,
        access_token,
        String,
    );
    impl_client_state!(
        has_refresh_token,
        refresh_token,
        clear_refresh_token,
        set_refresh_token,
        refresh_token,
        String,
    );
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}


#[derive(Debug)]
pub(crate) enum Error {
    Reqwest(reqwest::Error),
    JsonParsing(serde_json::Error),
    Authentication,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reqwest(e) => e.fmt(f),
            Self::JsonParsing(e) => e.fmt(f),
            Self::Authentication => write!(f, "authentication failed"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reqwest(e) => Some(e),
            Self::JsonParsing(e) => Some(e),
            Self::Authentication => None,
        }
    }
}
impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::JsonParsing(value)
    }
}


pub(crate) struct TokenAuthClient {
    client: Client,
    state: Mutex<ClientState>,

    auth_url: Url,
    basic_username: String,
    basic_password: String,
}
impl TokenAuthClient {
    pub fn new(auth_url: Url, basic_username: String, basic_password: String) -> Self {
        let client = Client::builder()
            .user_agent("elasticwave (github.com/RavuAlHemio/elasticwave)")
            .build()
            .expect("failed to build client");
        let state = Mutex::new(ClientState::new());
        Self {
            client,
            state,
            auth_url,
            basic_username,
            basic_password,
        }
    }

    pub async fn get(&self, url: &Url) -> Result<reqwest::Response, Error> {
        self.perform(&Method::GET, url, None).await
    }

    pub async fn perform(&self, method: &Method, url: &Url, body: Option<&[u8]>) -> Result<reqwest::Response, Error> {
        let mut state_guard = self.state.lock().await;

        if let Some(response) = self.try_perform_with_access_token(&mut *state_guard, method, url, body.clone()).await? {
            return Ok(response);
        }

        if let Some(response) = self.try_perform_after_refreshing_token(&mut *state_guard, method, url, body.clone()).await? {
            return Ok(response);
        }

        if let Some(response) = self.try_perform_after_fresh_login(&mut *state_guard, method, url, body).await? {
            return Ok(response);
        }

        Err(Error::Authentication)
    }

    async fn try_perform_with_access_token(&self, state: &mut ClientState, method: &Method, url: &Url, body: Option<&[u8]>) -> Result<Option<reqwest::Response>, Error> {
        if !state.has_access_token() {
            return Ok(None);
        }
        todo!();
    }

    async fn try_perform_after_refreshing_token(&self, state: &mut ClientState, method: &Method, url: &Url, body: Option<&[u8]>) -> Result<Option<reqwest::Response>, Error> {
        if !state.has_refresh_token() {
            return Ok(None);
        }
        todo!();
    }

    async fn try_perform_after_fresh_login(&self, state: &mut ClientState, method: &Method, url: &Url, body: Option<&[u8]>) -> Result<Option<reqwest::Response>, Error> {
        todo!();
    }

    fn get_basic_auth_header_value(&self) -> String {
        let base64_me = format!("{}:{}", self.basic_username, self.basic_password);
        let based = BASE64_STANDARD.encode(&base64_me);
        format!("Basic {}", based)
    }

    async fn try_refreshing_token(&self, state: &mut ClientState) -> Result<(), Error> {
        assert!(state.has_refresh_token());

        // prepare the body
        let refresh_value = serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": &state.refresh_token().unwrap(),
        });
        let refresh_string = serde_json::to_string(&refresh_value)
            .expect("failed to serialize token refresh data");
        let basic_auth = self.get_basic_auth_header_value();

        // apparently one has to pass WaveSuite credentials *and* the refresh token?!
        // (if we skip the refresh token, a new session is opened)
        let response = self.client
            .post(self.auth_url.clone())
            .header("Content-Type", "application/json")
            .header("Authorization", &basic_auth)
            .body(refresh_string)
            .send().await?;
        if response.status() == StatusCode::FORBIDDEN {
            // login data is not valid
            return Err(Error::Authentication);
        } else if response.status() == StatusCode::UNAUTHORIZED {
            // refresh token is not valid
            return Err(Error::Authentication);
        }
        response.error_for_status_ref()?;
        let response_string = response.text().await?;
        let response_token_pair: TokenPair = serde_json::from_str(&response_string)?;
        state.set_access_token(response_token_pair.access_token);
        state.set_refresh_token(response_token_pair.refresh_token);
        Ok(())
    }

    async fn try_logging_in(&self, state: &mut ClientState) -> Result<(), Error> {
        assert!(state.has_refresh_token());

        // prepare the body
        let login_value = serde_json::json!({
            "grant_type": "client_credentials",
        });
        let login_string = serde_json::to_string(&login_value)
            .expect("failed to serialize login data");
        let basic_auth = self.get_basic_auth_header_value();

        let response = self.client
            .post(self.auth_url.clone())
            .header("Content-Type", "application/json")
            .header("Authorization", &basic_auth)
            .body(login_string)
            .send().await?;
        if response.status() == StatusCode::FORBIDDEN {
            // login data is not valid
            return Err(Error::Authentication);
        }
        response.error_for_status_ref()?;
        let response_string = response.text().await?;
        let response_token_pair: TokenPair = serde_json::from_str(&response_string)?;
        state.set_access_token(response_token_pair.access_token);
        state.set_refresh_token(response_token_pair.refresh_token);
        Ok(())
    }
}
