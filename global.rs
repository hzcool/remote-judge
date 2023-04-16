use crate::judger::RemoteJudgeConfig;
use crate::server::ServerConfig;
use once_cell::sync::OnceCell;
use std::collections::HashMap;

static REMOTE_JUDGE_CONFIG_MAP: OnceCell<HashMap<String, RemoteJudgeConfig>> = OnceCell::new();
static SERVER_CONFIG: OnceCell<ServerConfig> = OnceCell::new();
static JUDGE_STATUS_MAP: OnceCell<HashMap<&'static str, HashMap<&'static str, &'static str>>> =
    OnceCell::new();

use simple_log::LogConfigBuilder;
use std::path::PathBuf;

pub async fn init_config(server_path: PathBuf, config_path: PathBuf, logger_path: PathBuf) {
    // 初始化服务配置
    let server_config_file = tokio::fs::read(server_path.as_path())
        .await
        .expect("不存在 server.json 文件");

    let server_config = serde_json::from_slice::<ServerConfig>(&server_config_file)
        .expect("解析 remote-judger 配置文件失败");

    SERVER_CONFIG.set(server_config).unwrap();

    //初始化 远程 oj 配置文件
    let rf_config_file = tokio::fs::read(config_path.as_path())
        .await
        .expect("不存在 remote_judge_config.json 文件");

    let config_map = serde_json::from_slice::<HashMap<String, RemoteJudgeConfig>>(&rf_config_file)
        .expect("解析 remote-judger 配置文件失败");

    REMOTE_JUDGE_CONFIG_MAP.set(config_map).unwrap();

    //日志配置
    let log_config = LogConfigBuilder::builder()
        .path(logger_path.as_path().to_str().unwrap())
        .level("info")
        .output_file()
        .build();
    simple_log::new(log_config).unwrap();

    //初始化状态映射表
    judge_status_map::judge_status_map_init();
}

pub fn remote_judge_config(remote_judger_name: &str) -> Option<&RemoteJudgeConfig> {
    REMOTE_JUDGE_CONFIG_MAP
        .get()
        .unwrap()
        .get(remote_judger_name)
}

pub fn server_config() -> &'static ServerConfig {
    SERVER_CONFIG.get().unwrap()
}

pub mod remote_judge_constant {
    pub mod names {
        pub const CODEFORCES: &str = "codeforces";
        pub const HDU: &str = "hdu";
        pub const GYM: &str = "gym";
        pub const ATCODER: &str = "atcoder";
    }
    pub mod base_url {
        pub const CODEFORCES: &str = "https://codeforces.com/";
        pub const HDU: &str = "https://acm.hdu.edu.cn/";
        pub const GYM: &str = "https://codeforces.com/";
        pub const ATCODER: &str = "https://atcoder.jp/";
    }
}

pub mod task_constant {
    pub mod names {
        pub const JUDGE: &str = "judge";
        pub const GET_PROBLEM: &str = "get_problem";
    }
}

pub mod judge_status {
    pub const AC: &str = "Accepted";
    pub const WA: &str = "Wrong Answer";
    pub const RE: &str = "Runtime Error";
    pub const TLE: &str = "Time Limit Exceeded";
    pub const MLE: &str = "Memory Limit Exceeded";
    pub const OLE: &str = "Output Limit Exceeded";
    pub const CE: &str = "Compile Error";
    pub const RN: &str = "Running";
    pub const WT: &str = "Waiting";
}

pub fn judge_status_map(remote_judger_name: &str) -> Option<&HashMap<&'static str, &'static str>> {
    JUDGE_STATUS_MAP.get().unwrap().get(remote_judger_name)
}

pub mod judge_status_map {
    use super::judge_status;
    use once_cell::sync::OnceCell;
    use std::collections::HashMap;
    pub static CODEFORCES_STATUS_MAP: OnceCell<HashMap<&'static str, &'static str>> =
        OnceCell::new();
    pub static HDU_STATUS_MAP: OnceCell<HashMap<&'static str, &'static str>> = OnceCell::new();
    pub static ATCODER_STATUS_MAP: OnceCell<HashMap<&'static str, &'static str>> = OnceCell::new();
    pub fn judge_status_map_init() {
        codeforces_status_map_init();
        hdu_status_map_init();
        atcoder_status_map_init();
    }

    fn codeforces_status_map_init() {
        let cf_map = vec![
            ("Accepted", judge_status::AC),
            ("Happy", judge_status::AC),
            ("Wrong answer", judge_status::WA),
            ("Runtime error", judge_status::RE),
            ("Time limit exceeded", judge_status::TLE),
            ("Memory limit exceeded", judge_status::MLE),
            ("Compilation error", judge_status::CE),
            ("Running", judge_status::RN),
            ("queue", judge_status::WT),
            ("Pending", judge_status::WT),
        ]
        .into_iter()
        .collect();
        CODEFORCES_STATUS_MAP.set(cf_map).unwrap();
    }

    fn hdu_status_map_init() {
        let hdu_map = vec![
            ("Accepted", judge_status::AC),
            ("Wrong Answer", judge_status::WA),
            ("Runtime Error", judge_status::RE),
            ("Time Limit Exceeded", judge_status::TLE),
            ("Memory Limit Exceeded", judge_status::MLE),
            ("Presentation Error", judge_status::WA),
            ("Output Limit Exceeded", judge_status::OLE),
            ("Compilation Error", judge_status::CE),
            ("Running", judge_status::RN),
            ("Queuing", judge_status::WT),
            ("Pending", judge_status::WT),
            ("Compiling", judge_status::WT),
        ]
        .into_iter()
        .collect();
        HDU_STATUS_MAP.set(hdu_map).unwrap();
    }

    fn atcoder_status_map_init() {
        let atocder_map = vec![
            ("AC", judge_status::AC),
            ("WA", judge_status::WA),
            ("TLE", judge_status::TLE),
            ("MLE", judge_status::MLE),
            ("RE", judge_status::RE),
            ("CE", judge_status::CE),
            ("OLE", judge_status::OLE),
            ("WJ", judge_status::WT),
            ("Judging", judge_status::RN),
        ]
        .into_iter()
        .collect();
        ATCODER_STATUS_MAP.set(atocder_map).unwrap();
    }
}
