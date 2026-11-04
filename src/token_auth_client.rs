use std::fmt;

use reqwest::{Client, Method};
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


#[derive(Debug)]
pub(crate) enum Error {
    Reqwest(reqwest::Error),
    Authentication,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reqwest(e) => e.fmt(f),
            Self::Authentication => write!(f, "authentication failed"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Reqwest(e) => Some(e),
            Error::Authentication => None,
        }
    }
}
impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
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
        todo!();
    }

    pub async fn get(&self, url: Url) -> Result<_, Error> {
        self.perform(Method::GET, url, None).await
    }

    pub async fn perform(&self, method: Method, url: Url, body: Option<_>) -> Result<_, Error> {
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

    async fn try_perform_with_access_token(&self, state: &mut ClientState, method: Method, url: Url, body: Option<_>) -> Result<Option<_>, Error> {
        if !state.has_access_token() {
            return Ok(None);
        }
        todo!();
    }

    async fn try_perform_after_refreshing_token(&self, state: &mut ClientState, method: Method, url: Url, body: Option<_>) -> Result<Option<_>, Error> {
        if !state.has_refresh_token() {
            return Ok(None);
        }
        todo!();
    }

    async fn try_perform_after_fresh_login(&self, state: &mut ClientState, method: Method, url: Url, body: Option<_>) -> Result<Option<_>, Error> {
        todo!();
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
        let response = self.client
            .post(self.auth_url.clone())
            .header("Content-Type", "application/json")
            .body(refresh_string)
            .send().await?;

        // apparently I have to pass WaveSuite credentials *and* the refresh token?!
        // what's the point of the refresh token if I can simply ask for an access token with only the WaveSuite creds?
        // => waiting for answer in Nokia ticket 06213883

        todo!();
    }
}
