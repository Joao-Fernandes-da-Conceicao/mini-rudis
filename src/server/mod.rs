/// TCP 服务器模块
pub mod handler;

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::db::Database;
use crate::script::ScriptEngine;

/// 服务器配置
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub db_count: usize,
    /// 后台过期键清理间隔（毫秒）
    pub eviction_interval_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 6379,
            db_count: 16,
            eviction_interval_ms: 100,
        }
    }
}

/// 启动 mini-rudis TCP 服务器
pub async fn run(config: ServerConfig) -> std::io::Result<()> {
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("mini-rudis listening on {}", addr);

    let db = Arc::new(Mutex::new(Database::new(config.db_count)));
    let script_engine = Arc::new(ScriptEngine::new());

    // 后台定期清理过期键的任务
    let db_bg = Arc::clone(&db);
    let interval_ms = config.eviction_interval_ms;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        loop {
            interval.tick().await;
            db_bg.lock().evict_expired();
        }
    });

    loop {
        let (socket, peer_addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                error!("accept error: {}", e);
                continue;
            }
        };
        info!("new connection: {}", peer_addr);

        let db = Arc::clone(&db);
        let script_engine = Arc::clone(&script_engine);
        tokio::spawn(async move {
            if let Err(e) = handler::handle_connection(socket, db, script_engine).await {
                // 客户端断开连接是正常情况，只记录非 EOF 错误
                let err_str = e.to_string();
                if !err_str.contains("connection reset") && !err_str.contains("eof") {
                    error!("connection error ({}): {}", peer_addr, e);
                }
            }
            info!("connection closed: {}", peer_addr);
        });
    }
}
