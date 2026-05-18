/// mini-rudis：用 Rust 实现的单机 Redis 兼容缓存服务器
///
/// # 功能特性
///
/// - **RESP 协议**：完整兼容 Redis 客户端（支持 redis-cli、各语言 SDK）
/// - **五种数据类型**：String、List、Hash、Set、ZSet（有序集合）
/// - **TTL / 过期机制**：惰性删除 + 后台定期主动淘汰
/// - **多逻辑库**：支持 SELECT 0-15
/// - **Lua 脚本**：EVAL / EVALSHA / SCRIPT 命令，`redis.call()` 可调用所有命令
/// - **持久化接口**：RDB / AOF 接口预留（未实现具体逻辑）
pub mod cmd;
pub mod db;
pub mod error;
pub mod persistence;
pub mod proto;
pub mod script;
pub mod server;
