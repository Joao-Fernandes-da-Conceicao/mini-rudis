/// mini-rudis 入口
///
/// 使用 `--help` 查看完整参数列表。
use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use mini_rudis::server::{run, ServerConfig};

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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // 初始化日志
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    fmt().with_env_filter(filter).init();

    info!("mini-rudis v{}", env!("CARGO_PKG_VERSION"));
    info!(
        "starting on {}:{} with {} databases",
        cli.host, cli.port, cli.databases
    );

    let config = ServerConfig {
        host: cli.host,
        port: cli.port,
        db_count: cli.databases.min(16),
        eviction_interval_ms: cli.eviction_interval_ms,
    };

    if let Err(e) = run(config).await {
        eprintln!("Server error: {e}");
        std::process::exit(1);
    }
}
