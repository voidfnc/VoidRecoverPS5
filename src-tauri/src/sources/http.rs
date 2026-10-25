//! HTTPS client used by the source adapters.
//!
//! Every request and every redirect hop must be https; redirects are capped.
//! Bodies are read in a stream with a byte limit so a misbehaving page can't
//! exhaust memory.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode};

use super::AdapterError;

pub const USER_AGENT: &str =
    concat!("VoidRecoverPS5/", env!("CARGO_PKG_VERSION"), " (+https://github.com/voidfnc/VoidRecoverPS5)");
const MAX_REDIRECTS: usize = 5;

fn redirect_policy() -> Policy {
    Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if attempt.url().scheme() != "https" {
            attempt.error("redirect to a non-https URL was refused")
        } else {
            attempt.follow()
        }
    })
}

pub fn build_client() -> Result<Client, AdapterError> {
    Client::builder()
        .user_agent(USER_AGENT)
        .redirect(redirect_policy())
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| AdapterError::Network(e.to_string()))
}

/// For the large package download: no whole-request timeout (a 1 GB file takes
/// minutes), but a stalled connection still fails after 30 seconds of silence.
pub fn build_download_client() -> Result<Client, AdapterError> {
    Client::builder()
        .user_agent(USER_AGENT)
        .redirect(redirect_policy())
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AdapterError::Network(e.to_string()))
}

pub struct TextResponse {
    pub final_url: String,
    pub body: String,
}

pub async fn fetch_text(client: &Client, url: &str, max_bytes: usize) -> Result<TextResponse, AdapterError> {
    let resp = client.get(url).send().await.map_err(net)?;
    let final_url = resp.url().to_string();
    if resp.status() != StatusCode::OK {
        return Err(AdapterError::Network(format!("{url} answered HTTP {}", resp.status().as_u16())));
    }
    if resp.content_length().is_some_and(|n| n as usize > max_bytes) {
        return Err(AdapterError::TooLarge { url: url.to_string(), limit: max_bytes });
    }
    let mut body = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(net)?;
        if body.len() + chunk.len() > max_bytes {
            return Err(AdapterError::TooLarge { url: url.to_string(), limit: max_bytes });
        }
        body.extend_from_slice(&chunk);
    }
    let body =
        String::from_utf8(body).map_err(|_| AdapterError::LayoutChanged(format!("{url} did not return UTF-8 text")))?;
    Ok(TextResponse { final_url, body })
}

pub struct HeadResponse {
    pub status: u16,
    pub content_length: Option<u64>,
    pub final_url: String,
}

pub async fn head(client: &Client, url: &str) -> Result<HeadResponse, AdapterError> {
    let resp = client.head(url).send().await.map_err(net)?;
    let content_length =
        resp.headers().get(reqwest::header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    Ok(HeadResponse { status: resp.status().as_u16(), content_length, final_url: resp.url().to_string() })
}

fn net(e: reqwest::Error) -> AdapterError {
    AdapterError::Network(e.to_string())
}

/// The host part of an https URL, for display ("final host").
pub fn host_of(url: &str) -> Option<String> {
    reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_identifies_the_app() {
        assert!(USER_AGENT.starts_with("VoidRecoverPS5/"));
    }

    #[test]
    fn host_extraction() {
        assert_eq!(
            host_of("https://pc.ps5.update.playstation.net/a/b").as_deref(),
            Some("pc.ps5.update.playstation.net")
        );
        assert_eq!(host_of("not a url"), None);
    }

    #[test]
    fn client_builds() {
        build_client().unwrap();
        build_download_client().unwrap();
    }
}
