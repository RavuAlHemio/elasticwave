use std::fmt;

use reqwest::{Certificate, Client};
use tracing::debug;
use url::Url;


#[derive(Debug)]
pub enum Error {
    HttpClient(reqwest::Error),
}
impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::HttpClient(value)
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AuthInfo {
    NoAuth,
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        password: String,
    },
}
impl AuthInfo {
    pub fn apply_to_builder(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            Self::NoAuth => {},
            Self::Bearer { token } => {
                builder = builder.bearer_auth(token);
            },
            Self::Basic { username, password } => {
                builder = builder.basic_auth(username, Some(password));
            },
        }
        builder
    }
}
impl fmt::Debug for AuthInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAuth => f
                .debug_struct("NoAuth")
                    .finish(),
            Self::Bearer { .. } => f
                .debug_struct("Bearer")
                    .field("token", &"[redacted]")
                    .finish(),
            Self::Basic { username, .. } => f
                .debug_struct("Basic")
                    .field("username", username)
                    .field("password", &"[redacted]")
                    .finish(),
        }
    }
}

pub struct ElasticClient {
    base_url: Url,
    auth_info: AuthInfo,
    client: reqwest::Client,
}
impl ElasticClient {
    pub fn new(
        base_url: Url,
        auth_info: AuthInfo,
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
        Self {
            base_url,
            auth_info,
            client,
        }
    }

    pub async fn upsert_document(&self, index: &str, id: &str, body: serde_json::Value) -> Result<(), Error> {
        let mut put_url = self.base_url.clone();
        let mut put_path_segs = put_url.path_segments_mut()
            .expect("base URL does not have path segments");
        put_path_segs.push(index);
        put_path_segs.push("_doc");
        put_path_segs.push(id);
        drop(put_path_segs);

        let body_string = serde_json::to_string(&body)
            .expect("failed to JSON-encode JSON value?!");

        // off we go
        let mut builder = self.client
            .put(put_url);
        builder = self.auth_info.apply_to_builder(builder);
        let response_res = builder
            .header("Content-Type", "application/json")
            .body(body_string)
            .send().await;
        match response_res {
            Ok(resp) => {
                let status = resp.status();
                let err_res = resp.error_for_status_ref()
                    .map(drop)
                    .map_err(Error::HttpClient);
                if let Ok(value) = resp.text().await {
                    debug!("Elastic response: [{}] {}", status, value);
                }
                err_res
            },
            Err(e) => Err(Error::HttpClient(e))
        }
    }

    pub async fn delete_document(&self, index: &str, id: &str) -> Result<(), Error> {
        let mut delete_url = self.base_url.clone();
        let mut delete_path_segs = delete_url.path_segments_mut()
            .expect("base URL does not have path segments");
        delete_path_segs.push(index);
        delete_path_segs.push("_doc");
        delete_path_segs.push(id);
        drop(delete_path_segs);

        let mut builder = self.client
            .delete(delete_url);
        builder = self.auth_info.apply_to_builder(builder);
        let response_res = builder
            .send().await;
        match response_res {
            Ok(resp) => {
                let status = resp.status();
                if let Ok(value) = resp.text().await {
                    debug!("Elastic response: [{}] {}", status, value);
                }
                Ok(())
            },
            Err(e) => Err(Error::HttpClient(e))
        }
    }

    pub async fn delete_all_documents(&self, index: &str) -> Result<(), Error> {
        let mut delete_url = self.base_url.clone();
        let mut delete_path_segs = delete_url.path_segments_mut()
            .expect("base URL does not have path segments");
        delete_path_segs.push(index);
        delete_path_segs.push("_delete_by_query");
        drop(delete_path_segs);

        let mut builder = self.client
            .delete(delete_url);
        builder = self.auth_info.apply_to_builder(builder);
        let response_res = builder
            .header("Content-Type", "application/json")
            .body("{\"query\":{\"match_all\":{}}}")
            .send().await;
        match response_res {
            Ok(resp) => {
                let status = resp.status();
                if let Ok(value) = resp.text().await {
                    debug!("Elastic response: [{}] {}", status, value);
                }
                Ok(())
            },
            Err(e) => Err(Error::HttpClient(e))
        }
    }
}
