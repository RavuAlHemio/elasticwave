use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use reqwest::{Certificate, Client, Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};
use url::Url;


macro_rules! impl_client_state_str {
    ($has_name:ident, $get_name:ident, $clear_name:ident, $set_name:ident, $prop_name:ident $(,)?) => {
        pub fn $has_name(&self) -> bool { self.$prop_name.is_some() }
        pub fn $get_name(&self) -> Option<&str> { self.$prop_name.as_ref().map(|s| s.as_str()) }
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

    impl_client_state_str!(
        has_access_token,
        access_token,
        clear_access_token,
        set_access_token,
        access_token,
    );
    impl_client_state_str!(
        has_refresh_token,
        refresh_token,
        clear_refresh_token,
        set_refresh_token,
        refresh_token,
    );

    pub fn to_token_pair(&self) -> Option<TokenPair> {
        if let Some(access_token) = self.access_token.as_ref().cloned() {
            if let Some(refresh_token) = self.refresh_token.as_ref().cloned() {
                return Some(TokenPair {
                    access_token,
                    refresh_token,
                });
            }
        }
        None
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug)]
pub(crate) enum Error {
    Reqwest(reqwest::Error),
    JsonParsing(serde_json::Error),
    Authentication(reqwest::Response),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reqwest(e) => e.fmt(f),
            Self::JsonParsing(e) => e.fmt(f),
            Self::Authentication(_) => write!(f, "authentication failed"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reqwest(e) => Some(e),
            Self::JsonParsing(e) => Some(e),
            Self::Authentication(_) => None,
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Body<'a> {
    pub content_type: &'a str,
    pub data: Cow<'a, [u8]>,
}

#[derive(Debug)]
pub(crate) enum ResponseResult {
    SuccessWithBody(Response),
    CredentialMissing,
    AuthenticationIssue(Response),
    ErrorStatus(Response),
    Error(Error),
}

#[derive(Debug)]
enum AuthResult {
    Success,
    CredentialMissing,
    AuthenticationIssue(Response),
    ErrorStatus(Response),
    Error(Error),
}

#[derive(Clone)]
pub(crate) struct TokenAuthClient {
    inner: Arc<TokenAuthClientInner>,
}
impl TokenAuthClient {
    pub fn new(
        auth_url: Url,
        logout_url: Url,
        basic_username: String,
        basic_password: String,
        custom_tls_certs: Option<&[Certificate]>,
        use_system_tls_certs: bool,
    ) -> Self {
        let inner_client = TokenAuthClientInner::new(
            auth_url,
            logout_url,
            basic_username,
            basic_password,
            custom_tls_certs,
            use_system_tls_certs,
        );
        Self {
            inner: Arc::new(inner_client),
        }
    }

    pub async fn get(&self, url: &Url) -> ResponseResult {
        self.perform(&Method::GET, url, None).await
    }

    pub async fn perform<'a>(&self, method: &Method, url: &Url, body: Option<Body<'a>>) -> ResponseResult {
        self.inner.perform(method, url, body).await
    }

    pub async fn get_token_pair(&self) -> Option<TokenPair> {
        let state_guard = self.inner.state.lock().await;
        state_guard.to_token_pair()
    }
}

struct TokenAuthClientInner {
    client: Client,
    state: Mutex<ClientState>,

    auth_url: Url,
    logout_url: Url,
    basic_username: String,
    basic_password: String,
}
impl TokenAuthClientInner {
    pub fn new(
        auth_url: Url,
        logout_url: Url,
        basic_username: String,
        basic_password: String,
        custom_tls_certs: Option<&[Certificate]>,
        use_system_tls_certs: bool,
    ) -> Self {
        let mut builder = Client::builder()
            .user_agent("elasticwave (github.com/RavuAlHemio/elasticwave)");
        match (custom_tls_certs, use_system_tls_certs) {
            (Some(certs), true) => {
                builder = builder
                    .tls_certs_merge(certs.into_iter().cloned());
            },
            (Some(certs), false) => {
                builder = builder
                    .tls_certs_only(certs.into_iter().cloned());
            },
            (None, true) => {
                // that's the default config
            },
            (None, false) => {
                // that's an empty list
                builder = builder
                    .tls_certs_only([]);
            },
        }
        let client = builder.build()
            .expect("failed to build client");
        let state = Mutex::new(ClientState::new());
        Self {
            client,
            state,
            auth_url,
            logout_url,
            basic_username,
            basic_password,
        }
    }

    pub async fn perform<'a>(&self, method: &Method, url: &Url, body: Option<Body<'a>>) -> ResponseResult {
        let mut state_guard = self.state.lock().await;

        debug!("attempting to perform with access token");
        match self.try_perform_with_access_token(&mut *state_guard, method, url, body.clone()).await {
            // pass these up
            ResponseResult::SuccessWithBody(resp) => return ResponseResult::SuccessWithBody(resp),
            ResponseResult::ErrorStatus(resp) => return ResponseResult::ErrorStatus(resp),
            ResponseResult::Error(e) => return ResponseResult::Error(e),

            ResponseResult::AuthenticationIssue(_)|ResponseResult::CredentialMissing => {
                // try the next method
            },
        }

        debug!("attempting to perform after refreshing token");
        match self.try_perform_after_refreshing_token(&mut *state_guard, method, url, body.clone()).await {
            // pass these up
            ResponseResult::SuccessWithBody(resp) => return ResponseResult::SuccessWithBody(resp),
            ResponseResult::ErrorStatus(resp) => return ResponseResult::ErrorStatus(resp),
            ResponseResult::Error(e) => return ResponseResult::Error(e),

            ResponseResult::AuthenticationIssue(_)|ResponseResult::CredentialMissing => {
                // try the next method
            },
        }

        // pass this one up finally
        debug!("attempting to perform after fresh login");
        self.try_perform_after_fresh_login(&mut *state_guard, method, url, body).await
    }

    pub async fn logout<'a>(&self) -> ResponseResult {
        let mut state_guard = self.state.lock().await;

        let access_token = match state_guard.access_token() {
            Some(at) => at,
            None => {
                warn!("cannot log out without access token");
                return ResponseResult::CredentialMissing;
            }
        };
        let access_token_encoded = urlencoding::encode(access_token);
        let request_body = format!("token={}", access_token_encoded);

        debug!("logging out");
        let response_res = self.client
            .request(Method::POST, self.logout_url.clone())
            .basic_auth(&self.basic_username, Some(&self.basic_password))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(request_body)
            .send().await;
        let response = match response_res {
            Ok(r) => r,
            Err(e) => {
                error!("error performing logout: {}", e);
                return ResponseResult::Error(e.into());
            },
        };
        if let Err(e) = response.error_for_status_ref() {
            error!("error response when performing logout: {}", e);
            return ResponseResult::ErrorStatus(response);
        }

        // logged out => these tokens are no longer valid
        state_guard.clear_access_token();
        state_guard.clear_refresh_token();

        ResponseResult::SuccessWithBody(response)
    }

    async fn try_perform_with_access_token<'a>(&self, state: &mut ClientState, method: &Method, url: &Url, body: Option<Body<'a>>) -> ResponseResult {
        let access_token = match state.access_token() {
            Some(at) => at,
            None => {
                debug!("cannot perform request with access token as there is none");
                return ResponseResult::CredentialMissing;
            },
        };

        let mut builder = self.client
            .request(method.clone(), url.clone())
            .bearer_auth(access_token);
        if let Some(b) = body {
            builder = builder
                .header("Content-Type", b.content_type)
                .body(b.data.to_vec());
        }
        let response = match builder.send().await {
            Ok(r) => r,
            Err(e) => {
                debug!("error performing request with access token: {}", e);
                return ResponseResult::Error(e.into());
            },
        };
        match response.status() {
            StatusCode::FORBIDDEN|StatusCode::UNAUTHORIZED => {
                debug!("authentication failure while performing request with access token");
                state.clear_access_token();
                ResponseResult::AuthenticationIssue(response)
            },
            _ => ResponseResult::SuccessWithBody(response),
        }
    }

    async fn try_perform_after_refreshing_token<'a>(&self, state: &mut ClientState, method: &Method, url: &Url, body: Option<Body<'a>>) -> ResponseResult {
        if !state.has_refresh_token() {
            debug!("cannot perform request with access token as there is none");
            return ResponseResult::CredentialMissing;
        }

        // perform a refresh
        match self.try_refreshing_token(state).await {
            AuthResult::Success => {
                // OK, now try to perform the operation with the new access token
                // (but pass any authentication issues upward so we don't loop forever)
                debug!("token refresh appears to have succeeded");
                self.try_perform_with_access_token(state, method, url, body).await
            },

            // pass these upward
            AuthResult::AuthenticationIssue(r) => {
                debug!("refresh failed with authentication issue");
                ResponseResult::AuthenticationIssue(r)
            },
            AuthResult::CredentialMissing => {
                debug!("refresh failed due to a missing credential");
                ResponseResult::CredentialMissing
            },
            AuthResult::ErrorStatus(r) => {
                debug!("refresh failed due to an error response");
                ResponseResult::ErrorStatus(r)
            },
            AuthResult::Error(e) => {
                debug!("refresh failed due to a lower-level error: {}", e);
                ResponseResult::Error(e)
            },
        }
    }

    async fn try_perform_after_fresh_login<'a>(&self, state: &mut ClientState, method: &Method, url: &Url, body: Option<Body<'a>>) -> ResponseResult {
        // perform a new login
        match self.try_logging_in(state).await {
            AuthResult::Success => {
                // OK, now try to perform the operation with the new access token
                // (but pass any authentication issues upward so we don't loop forever)
                debug!("fresh login appears to have succeeded");
                self.try_perform_with_access_token(state, method, url, body).await
            },

            // pass these upward
            AuthResult::AuthenticationIssue(r) => {
                debug!("refresh failed with authentication issue");
                ResponseResult::AuthenticationIssue(r)
            },
            AuthResult::CredentialMissing => {
                debug!("refresh failed due to a missing credential");
                ResponseResult::CredentialMissing
            },
            AuthResult::ErrorStatus(r) => {
                debug!("refresh failed due to an error response");
                ResponseResult::ErrorStatus(r)
            },
            AuthResult::Error(e) => {
                debug!("refresh failed due to a lower-level error: {}", e);
                ResponseResult::Error(e)
            },
        }
    }

    async fn try_refreshing_token(&self, state: &mut ClientState) -> AuthResult {
        assert!(state.has_refresh_token());

        // prepare the body
        let refresh_value = serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": &state.refresh_token().unwrap(),
        });
        let refresh_string = serde_json::to_string(&refresh_value)
            .expect("failed to serialize token refresh data");

        // apparently one has to pass WaveSuite credentials *and* the refresh token?!
        // (if we skip the refresh token, a new session is opened)
        let response_res = self.client
            .post(self.auth_url.clone())
            .header("Content-Type", "application/json")
            .basic_auth(&self.basic_username, Some(&self.basic_password))
            .body(refresh_string)
            .send().await;
        let response = match response_res {
            Ok(r) => r,
            Err(e) => return AuthResult::Error(e.into()),
        };
        if response.status() == StatusCode::FORBIDDEN || response.status() == StatusCode::UNAUTHORIZED {
            // FORBIDDEN: login data not valid
            // UNAUTHORIZED: refresh token not (no longer?) valid
            return AuthResult::AuthenticationIssue(response);
        }
        if let Err(e) = response.error_for_status_ref() {
            return AuthResult::Error(e.into());
        }
        let response_string = match response.text().await {
            Ok(rs) => rs,
            Err(e) => return AuthResult::Error(e.into()),
        };
        let response_token_pair: TokenPair = match serde_json::from_str(&response_string) {
            Ok(rs) => rs,
            Err(e) => return AuthResult::Error(e.into()),
        };
        state.set_access_token(response_token_pair.access_token);
        state.set_refresh_token(response_token_pair.refresh_token);
        AuthResult::Success
    }

    async fn try_logging_in(&self, state: &mut ClientState) -> AuthResult {
        // prepare the body
        let login_value = serde_json::json!({
            "grant_type": "client_credentials",
        });
        let login_string = serde_json::to_string(&login_value)
            .expect("failed to serialize login data");

        let response_res = self.client
            .post(self.auth_url.clone())
            .header("Content-Type", "application/json")
            .basic_auth(&self.basic_username, Some(&self.basic_password))
            .body(login_string)
            .send().await;
        let response = match response_res {
            Ok(r) => r,
            Err(e) => return AuthResult::Error(e.into()),
        };
        if response.status() == StatusCode::FORBIDDEN {
            // login data is not valid
            return AuthResult::AuthenticationIssue(response);
        }
        if let Err(e) = response.error_for_status_ref() {
            return AuthResult::Error(e.into());
        }
        let response_string = match response.text().await {
            Ok(rs) => rs,
            Err(e) => return AuthResult::Error(e.into()),
        };
        let response_token_pair: TokenPair = match serde_json::from_str(&response_string) {
            Ok(rs) => rs,
            Err(e) => return AuthResult::Error(e.into()),
        };
        state.set_access_token(response_token_pair.access_token);
        state.set_refresh_token(response_token_pair.refresh_token);
        AuthResult::Success
    }
}
