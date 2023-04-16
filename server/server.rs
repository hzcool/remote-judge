use crate::{
    global::{self, remote_judge_constant::names as remote_judge_names},
    judger,
};

use anyhow::anyhow;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use hyper::Request;
use simple_log::log::info;

use super::{task, WsRequest};

pub async fn make_ws_server() {
    let config = global::server_config();
    let ws_addr = format!("{}:{}", config.host, config.ws_port);

    info!("web-socket服务: {}", ws_addr);
    let router = Router::new()
        .route("/entry", get(web_socket_handler))
        .route_layer(middleware::from_fn(check_access_token));

    axum::Server::bind(&ws_addr.parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}

async fn check_access_token<B>(req: Request<B>, next: Next<B>) -> Response {
    let token = global::server_config().access_token.as_ref();
    if token.is_none() {
        return next.run(req).await;
    }
    if let Some(token_header) = req.headers().get("ACCESS_TOKEN") {
        if let Ok(access_token) = token_header.to_str() {
            if access_token == token.unwrap() {
                return next.run(req).await;
            }
        }
    }
    serde_json::json!({"error": "没有权限"})
        .to_string()
        .into_response()
}

async fn web_socket_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|mut socket| async move {
        let Some(Ok(msg)) = socket.recv().await else {
            let _ = socket.send( Message::Text(serde_json::json!({"error": "获取 web-socket数据失败"}).to_string()
        )).await;
            return ;
        };
        
        let req = match serde_json::from_slice::<WsRequest>(msg.into_data().as_slice()) {
            Ok(_conf) => _conf,
            Err(_) => {
                let _ = socket.send( Message::Text(serde_json::json!({"error": "请求数据错误"}).to_string())).await;
                return;
            }
        };

        info!("{:?}", req);
        
        if let Err(e) = dispatch(&mut socket, req).await {
            let _ = socket.send( Message::Text(serde_json::json!({"error": format!("{}", e)}).to_string())).await;
        }
    })
}

async fn dispatch(ws: &mut WebSocket, req: WsRequest) -> anyhow::Result<()> {
    match req.remote_judge.as_str() {
        remote_judge_names::CODEFORCES => {
            task::run(&judger::Codeforces::new(false).await?, ws, req).await
        }
        remote_judge_names::GYM => task::run(&judger::Codeforces::new(true).await?, ws, req).await,
        remote_judge_names::HDU => task::run(&judger::Hdu::new().await?, ws, req).await,
        remote_judge_names::ATCODER => task::run(&judger::Atcoder::new().await?, ws, req).await,
        _ => Err(anyhow!("请求类型错误")),
    }
}
