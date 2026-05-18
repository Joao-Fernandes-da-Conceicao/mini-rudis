/// mini-rudis 入口
///
/// 使用 **`current_thread` runtime + [`tokio::task::LocalSet`]**：单线程调度所有异步任务，
/// 配合 `Rc<RefCell<Database>>` 实现命令路径无互斥锁（对齐 Redis 「单线程执行命令」模型）。
///
/// # 不要使用 `#[tokio::main]`
///
/// 多线程运行时默认下的 `spawn` 要求 `Send`，且无法与安全使用 `Rc`/`spawn_local` 的模型混用。
use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use mini_rudis::server::{run, ServerConfig};
use tokio::task::LocalSet;

/// mini-rudis：Rust 实现的单机 Redis 兼容缓存服务器
#[derive(Parser, Debug)]
#[command(name = "mini-rudis", version, about, long_about = None)]
struct Cli {
    /// 监听地址
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// 监听端口
    #[arg(short, long, default_value_t = 6379)]
    port: u16,

    /// 逻辑数据库数量（默认 16，最大 16）
    #[arg(long, default_value_t = 16)]
    databases: usize,

    /// 日志级别（trace/debug/info/warn/error）
    #[arg(long, default_value = "info")]
    log_level: String,

    /// 后台过期键清理间隔（毫秒）
    #[arg(long, default_value_t = 100)]
    eviction_interval_ms: u64,
}

fn main() {
    let cli = Cli::parse();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    fmt().with_env_filter(filter).init();

    info!("mini-rudis v{}", env!("CARGO_PKG_VERSION"));

    let config = ServerConfig {
        host: cli.host,
        port: cli.port,
        db_count: cli.databases.min(16),
        eviction_interval_ms: cli.eviction_interval_ms,
    };

    info!(
        "starting single-thread dispatcher on {}:{} ({} databases)",
        config.host, config.port, config.db_count
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime");

    let local = LocalSet::new();
    local.block_on(&rt, async move {
        if let Err(e) = run(config).await {
            eprintln!("Server error: {e}");
            std::process::exit(1);
        }
    });
}
