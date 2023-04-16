use super::utils::request::RemoteJudgeRequest;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Deserialize, Serialize)]
pub struct Account {
    pub handler: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RemoteJudgeConfig {
    pub lang_map: HashMap<String, String>,
    pub accounts: Vec<Account>,
}

pub struct Handler {
    pub username: String,
    pub password: String,
    pub req: RemoteJudgeRequest,
    pub mtx: Mutex<HashSet<String>>,
    pub notify: Notify,
}

impl Handler {
    pub fn new(username: String, password: String, url: &'static str) -> Self {
        Self {
            username,
            password,
            req: RemoteJudgeRequest::new(url),
            mtx: Mutex::new(HashSet::new()),
            notify: Notify::new(),
        }
    }

    pub async fn accquire(&self, key: &str) {
        loop {
            let mut guard = self.mtx.lock().await;
            if guard.contains(key) {
                drop(guard);
                self.notify.notified().await;
            } else {
                guard.insert(key.into());
                break;
            }
        }
    }

    pub async fn release(&self, key: &str) {
        let mut guard = self.mtx.lock().await;
        guard.remove(key);
        self.notify.notify_one();
    }
}


