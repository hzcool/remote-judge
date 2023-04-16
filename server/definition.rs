use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub ws_port: String,
    pub access_token: Option<String>,
    pub max_poll_times: usize,
    pub max_wait_time: u32,
    pub wait_incr: u32,
    pub wait_base: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WsRequest {
    pub remote_judge: String,
    pub request_type: String, // judge

    pub lang: Option<String>,
    pub problem_id: Option<String>,
    pub source: Option<String>,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_req() {
        let req = WsRequest {
            remote_judge: "atcoder".into(),
            request_type: "judge".into(),
            lang: Some("CPP".into()),
            problem_id: Some("arc159_f".into()),
            source: Some(include_str!("../code.txt").into()),
        };
        println!("{}", serde_json::json!(req).to_string());
    }
}
