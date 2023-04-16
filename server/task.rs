use super::WsRequest;
use crate::global::{
    remote_judge_config, remote_judge_constant::names as remote_judge_names, server_config,
    task_constant::names as task_names,
};
use crate::judger::provider::Provider;
use anyhow::anyhow;
use axum::extract::ws::{Message, WebSocket};

async fn judge_task<T: ?Sized + Provider>(
    provider: &T,
    ws: &mut WebSocket,
    req: WsRequest,
) -> anyhow::Result<()> {
    if req.problem_id.is_none() || req.source.is_none() || req.lang.is_none() {
        return Err(anyhow!("请求参数错误"));
    }
    let problem_id = req.problem_id.as_ref().unwrap();
    let source = req.source.as_ref().unwrap();
    let lang = req.lang.as_ref().unwrap();

    let Some(rj_config) = remote_judge_config(if req.remote_judge == remote_judge_names::GYM {remote_judge_names::CODEFORCES} else { &req.remote_judge }) else {
        return Err(anyhow!("不支持 该 OJ 测评"));
    };

    let Some(lang_id) =  rj_config.lang_map.get(lang) else {
        return Err(anyhow!("不支持该语言"));
    };

    let submission_id = provider.submit_code(problem_id, source, lang_id).await?;
    ws.send(Message::Text(
        serde_json::json!({
            "submission_id": submission_id,
            "info": provider.get_handler_info()
        })
        .to_string(),
    ))
    .await?;

    let config = server_config();

    let mut pre_info = "".to_string();
    let mut sleep_time = config.wait_base;

    for _ in 0..config.max_poll_times {
        match provider.poll(&submission_id).await {
            Err(_) => {
                sleep_time = std::cmp::min(sleep_time + config.wait_incr, config.max_wait_time)
            }
            Ok(res) => {
                if res.info != pre_info {
                    ws.send(Message::Text(serde_json::json!(res).to_string()))
                        .await?;
                    if res.is_over {
                        return Ok(());
                    }
                    sleep_time = config.wait_base;
                    pre_info = res.info;
                } else {
                    sleep_time = std::cmp::min(sleep_time + config.wait_incr, config.max_wait_time);
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(sleep_time as u64)).await
    }

    Err(anyhow!("超过最大重试次数， 请查看远程测评网站是否不可访问"))
}

async fn get_problem_task<T: ?Sized + Provider>(
    provider: &T,
    ws: &mut WebSocket,
    req: WsRequest,
) -> anyhow::Result<()> {
    if req.problem_id.is_none() {
        return Err(anyhow!("请求参数错误"));
    }
    let problem_id = req.problem_id.as_ref().unwrap();
    let res = provider.get_problem(problem_id).await?;
    let _ = ws
        .send(Message::Text(serde_json::json!(res).to_string()))
        .await;
    Ok(())
}

pub async fn run<T: ?Sized + Provider>(
    provider: &T,
    ws: &mut WebSocket,
    req: WsRequest,
) -> anyhow::Result<()> {
    match req.request_type.as_str() {
        task_names::JUDGE => judge_task(provider, ws, req).await,
        task_names::GET_PROBLEM => get_problem_task(provider, ws, req).await,
        _ => Err(anyhow!("任务类型错误")),
    }
}
