/// TCP 服务器模块
///
/// 使用 **`tokio` current-thread scheduler + `spawn_local`**：所有连接任务与过期淘汰在同一线程
/// 上轮询，数据库为 `Rc<RefCell<Database>>`，**命令路径无 Mutex**，与 Redis 单线程模型一致。
pub mod handler;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tokio::net::TcpListener;
use tracing::{error, info};

use crate::db::Database;
use crate::script::ScriptEngine;
use crate::SharedDb;

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

/// 必须在 [`tokio::task::LocalSet`] 上下文中 `.await` 调用。
pub async fn run(config: ServerConfig) -> std::io::Result<()> {
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!(
        "mini-rudis listening on {} (single-thread executor, lock-free command path)",
        addr
    );

    let db: SharedDb = Rc::new(RefCell::new(Database::new(config.db_count)));
    let script_engine = Rc::new(ScriptEngine::new());

    let db_bg = Rc::clone(&db);
    let interval_ms = config.eviction_interval_ms;
    tokio::task::spawn_local(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        loop {
            interval.tick().await;
            db_bg.borrow_mut().evict_expired();
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

        let db_conn = Rc::clone(&db);
        let se = Rc::clone(&script_engine);
        tokio::task::spawn_local(async move {
            if let Err(e) = handler::handle_connection(socket, db_conn, se).await {
                let err_str = e.to_string();
                if !err_str.contains("connection reset") && !err_str.contains("eof") {
                    error!("connection error ({}): {}", peer_addr, e);
                }
            }
            info!("connection closed: {}", peer_addr);
        });
    }
}
