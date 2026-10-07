use std::time::Duration;

use reqwest::{
    blocking::multipart,
    header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue},
};
use rvimage_domain::{RvResult, rverr, to_rv};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::result::trace_ok_err;

/// A JSON object is interpreted as multiple headers, anything else as authorization.
/// Non-string values, including nested objects, are sent as compact JSON.
fn insert_headers(headers: &mut HeaderMap, s: &str) {
    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, Value>>(s) {
        for (k, v) in map {
            let v = match v {
                Value::String(v) => v,
                v => v.to_string(),
            };
            if let (Some(k), Some(v)) = (
                trace_ok_err(HeaderName::from_bytes(k.as_bytes())),
                trace_ok_err(HeaderValue::from_str(&v)),
            ) {
                headers.insert(k, v);
            }
        }
    } else if let Some(v) = trace_ok_err(HeaderValue::from_str(s)) {
        headers.insert(AUTHORIZATION, v);
    }
}

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
        if let Some(s) = authorization {
            insert_headers(&mut headers, s);
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
    /// Headers with the same name as existing ones replace them.
    pub fn add_headers(&mut self, s: &str) {
        insert_headers(&mut self.headers, s);
    }
    pub fn send<Q, O>(&self, form: multipart::Form, query_params: Option<&Q>) -> RvResult<O>
    where
        Q: Serialize,
        O: DeserializeOwned,
    {
        tracing::info!("Sending predictive labeling request to {}", self.url);

        let request = self.client.post(&self.url).headers(self.headers.clone());
        let request = if let Some(qp) = query_params {
            request.query(qp)
        } else {
            request
        };
        let response = request
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

#[cfg(test)]
use crate::cmd_runner::{NO_ENV, run_cmd};

#[test]
fn test_headers() {
    let rd = RestData::new("http://x".into(), Some("Bearer abc"), 1, "ep");
    assert_eq!(rd.headers.len(), 1);
    assert_eq!(rd.headers.get(AUTHORIZATION).unwrap(), "Bearer abc");

    let rd = RestData::new(
        "http://x".into(),
        Some(r#"{"Authorization": "Bearer abc", "X-Api-Key": "xyz"}"#),
        1,
        "ep",
    );
    assert_eq!(rd.headers.len(), 2);
    assert_eq!(rd.headers.get(AUTHORIZATION).unwrap(), "Bearer abc");
    assert_eq!(rd.headers.get("x-api-key").unwrap(), "xyz");

    let rd = RestData::new(
        "http://x".into(),
        Some(
            r#"{"X-Retry": 3, "X-Debug": true, "X-Nested": {"a": "b"}, "X-List": [1, 2], "X-Null": null}"#,
        ),
        1,
        "ep",
    );
    assert_eq!(rd.headers.len(), 5);
    assert_eq!(rd.headers.get("x-retry").unwrap(), "3");
    assert_eq!(rd.headers.get("x-debug").unwrap(), "true");
    assert_eq!(rd.headers.get("x-nested").unwrap(), r#"{"a":"b"}"#);
    assert_eq!(rd.headers.get("x-list").unwrap(), "[1,2]");
    assert_eq!(rd.headers.get("x-null").unwrap(), "null");
}

#[test]
fn test_headers_cmd() {
    let prj_folder = std::env::temp_dir().join("rvimage_test_headers_cmd");
    std::fs::create_dir_all(prj_folder.join("scripts")).unwrap();
    let prj_path = prj_folder.join("prj.rvi");
    let cmd = if cfg!(windows) {
        std::fs::write(
            prj_folder.join("scripts").join("token.bat"),
            "@echo Bearer fromscript",
        )
        .unwrap();
        "cmd /C scripts\\token.bat"
    } else {
        std::fs::write(
            prj_folder.join("scripts").join("token.sh"),
            "echo Bearer fromscript",
        )
        .unwrap();
        "sh scripts/token.sh"
    };
    let mut rd = RestData::new(
        "http://x".into(),
        Some(r#"{"Authorization": "Bearer abc", "X-Api-Key": "xyz"}"#),
        1,
        "ep",
    );
    let cmd_out = run_cmd(cmd, &[], &prj_path, false, None, NO_ENV);
    std::fs::remove_dir_all(&prj_folder).unwrap();
    rd.add_headers(&cmd_out.unwrap());
    assert_eq!(rd.headers.len(), 2);
    assert_eq!(rd.headers.get(AUTHORIZATION).unwrap(), "Bearer fromscript");
    assert_eq!(rd.headers.get("x-api-key").unwrap(), "xyz");
    assert!(run_cmd("  ", &[], &prj_path, false, None, NO_ENV).is_err());
    assert!(
        run_cmd(
            "rvimage-nonexistent-prg",
            &[],
            &prj_path,
            false,
            None,
            NO_ENV
        )
        .is_err()
    );
}
