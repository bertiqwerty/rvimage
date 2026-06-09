use std::time::Duration;

use reqwest::{
    blocking::multipart,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use rvimage_domain::{RvResult, rverr, to_rv};
use serde::de::DeserializeOwned;

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

        let url = if url.split('/').next_back() == Some(endpoint) {
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
    pub fn send<O>(&self, form: multipart::Form, query_params: Option<&str>) -> RvResult<O>
    where
        O: DeserializeOwned,
    {
        tracing::info!("Sending predictive labeling request to {}", self.url);
        let url_with_query = query_params.map(|qp| format!("{}?{}", self.url, qp));
        let url = if let Some(url_wq) = &url_with_query {
            tracing::info!("Full URL with query parameters: {url_wq}");
            url_wq
        } else {
            tracing::info!("Using base URL, no query parameters provided: {}", self.url);
            &self.url
        };
        let response = self
            .client
            .post(url)
            .headers(self.headers.clone())
            .multipart(form)
            .timeout(Duration::from_millis(self.timeout_ms as u64))
            .send()
            .map_err(to_rv)?;
        if response.status().is_success() {
            let segs = response.json::<O>().map_err(to_rv)?;
            Ok(segs)
        } else {
            let status = response.status();
            let err_msg = response
                .text()
                .unwrap_or("no error message available".into());
            Err(rverr!(
                "predictive labelling failed with status {} and error message '{}'",
                status,
                err_msg
            ))
        }
    }
}
