#![feature(async_fn_in_trait)]
use std::str::FromStr;

pub mod global;
pub mod judger;
pub mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let (server_path, config_path, logger_path) = if args.len() >= 4 {
        (
            std::path::PathBuf::from_str(&args[1]).unwrap(),
            std::path::PathBuf::from_str(&args[2]).unwrap(),
            std::path::PathBuf::from_str(&args[3]).unwrap(),
        )
    } else {
        (
            std::path::PathBuf::from_str("server.json").unwrap(),
            std::path::PathBuf::from_str("remote_judge_config.json").unwrap(),
            std::path::PathBuf::from_str("remote_judge.log").unwrap(),
        )
    };

    global::init_config(server_path, config_path, logger_path).await;
    server::make_ws_server().await;
    Ok(())
}
