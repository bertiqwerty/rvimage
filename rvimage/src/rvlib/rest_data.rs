use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use crate::result::trace_ok_err;

pub struct RestData {
    pub url: String,
    pub headers: HeaderMap,
    pub client: reqwest::blocking::Client,
    pub timeout_ms: usize,
}
impl RestData {
    pub fn new(
        mut url: String,
        authorization: Option<&str>,
        timeout_ms: usize,
        endpoint: &str,
    ) -> Self {
        let client = reqwest::blocking::Client::new();
        let mut headers = HeaderMap::new();
        if let Some(s) = authorization
            && let Some(s) = trace_ok_err(HeaderValue::from_str(s))
        {
            headers.insert(AUTHORIZATION, s);
        }
        while url.ends_with('/') && !url.is_empty() {
            url = url[..url.len() - 1].into();
        }

        let url = if url.split('/').next_back() == Some("predict") {
            url
        } else {
            format!("{url}/{endpoint}")
        };

        Self {
            url,
            headers,
            client,
            timeout_ms,
        }
    }
}
