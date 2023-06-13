use std::collections::HashMap;

use anyhow::Result;
use reqwest::{header, Client};
use serde::Serialize;

pub struct PostConfig {
    headers: header::HeaderMap,
    timeout: Option<std::time::Duration>,
}
impl PostConfig {
    pub fn new(headers: header::HeaderMap, timeout: Option<std::time::Duration>) -> Self {
        Self { headers, timeout }
    }
}

fn default_header() -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    [
        ("Accept", header::HeaderValue::from_static("*/*")),
        ("Connection", header::HeaderValue::from_static("keep-alive")),
    ]
    .into_iter()
    .for_each(|(x, y)| {
        headers.insert(x, y);
    });
    headers
}

#[derive(Debug)]
pub struct RemoteJudgeRequest {
    pub client: Client,
    pub base_url: &'static str,
}

impl RemoteJudgeRequest {
    pub fn new(base_url: &'static str) -> Self {
        Self {
            client:  Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36 Edg/108.0.1462.15")
            .timeout(std::time::Duration::from_secs(10))
            .cookie_store(true)
            .default_headers(default_header())
            .build()
            .expect("创建 client 失败"),
            base_url
        }
    }

    pub fn get_url(&self, url: &str) -> String {
        if url.starts_with("http") {
            return url.into();
        }

        let mut res = self.base_url.to_string();

        if !res.ends_with("/") {
            res.push_str("/")
        }

        if url.starts_with("/") {
            res.push_str(&url[1..])
        } else {
            res.push_str(url)
        }
        res
    }

    pub async fn get(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        Ok(self.client.get(self.get_url(url)).send().await?)
    }

    pub fn get_cookie_kv(&self) -> HashMap<String, String> {
        let Result::Ok(q) = self.client.get(self.base_url).build() else {
            return HashMap::default();
        };

        println!("{:?}", q.headers());

        let Some(cookie_header) = q.headers().get("cookie") else {
            return HashMap::default();
        };

        cookie_header
            .to_str()
            .unwrap_or_default()
            .split("; ")
            .map(|c| {
                let pos = c.find("=").unwrap_or(0);
                (c[..pos].to_string(), c[pos + 1..].to_string())
            })
            .collect()
    }

    pub async fn post<T: Serialize + ?Sized>(
        &self,
        url: &str,
        data: &T,
    ) -> anyhow::Result<reqwest::Response> {
        Ok(self
            .client
            .post(self.get_url(url))
            .form(data)
            .send()
            .await?)
    }

    pub async fn post_with_config<T: Serialize + ?Sized>(
        &self,
        url: &str,
        data: &T,
        config: PostConfig,
    ) -> anyhow::Result<reqwest::Response> {
        let mut req = self
            .client
            .post(self.get_url(url))
            .form(data)
            .headers(config.headers);
        if let Some(t) = config.timeout {
            req = req.timeout(t);
        }
        Ok(req.send().await?)
    }
}
