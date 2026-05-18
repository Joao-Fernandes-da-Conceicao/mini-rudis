# mini-rudis 设计文档

## 项目概述

mini-rudis 是一个用 Rust 实现的单机 Redis 兼容缓存服务器，支持 RESP 协议和五种核心数据类型。

## 架构设计

```
┌─────────────────────────────────────────────────────┐
│                   客户端 (redis-cli / SDK)           │
└──────────────────────┬──────────────────────────────┘
                       │ TCP / RESP
┌──────────────────────▼──────────────────────────────┐
│              server::handler（连接处理层）            │
│   • 每连接一个 Tokio 任务                            │
│   • 维护独立的 db_index（当前逻辑库）                │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│              proto（RESP 协议层）                    │
│   • parse_frame()：BytesMut → Frame                 │
│   • Frame::serialize()：Frame → Vec<u8>             │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│              cmd（命令层）                           │
│   • parse()：Vec<Bytes> → Command 枚举              │
│   • execute()：Command → Frame                      │
│   • 按数据类型分模块：string/list/hash/set/zset     │
└──────────┬───────────┴──────────────────────────────┘
           │                      │
┌──────────▼──────────┐  ┌────────▼────────────────────┐
│   db（数据库层）    │  │  script（Lua 脚本引擎）      │
│   • 16 个逻辑数据库 │  │  • eval() / evalsha()       │
│   • TTL 惰性过期    │  │  • redis.call() 回调        │
│   • 后台主动淘汰    │  │  • 脚本 SHA1 缓存           │
└─────────────────────┘  └─────────────────────────────┘
```

## 数据结构设计

### String
直接存储为 `Vec<u8>`，支持二进制安全。整数操作通过字符串解析实现。

### List
使用 `VecDeque<Vec<u8>>`，两端均为 O(1) 的 push/pop，适合实现队列和栈。

### Hash
使用 `HashMap<String, Vec<u8>>`，field 到 value 的映射，O(1) 查找。

### Set
使用 `HashSet<String>`，O(1) 成员判断，支持集合运算（并/交/差）。

### ZSet（有序集合）
双索引设计：
```rust
pub struct ZSetInner {
    scores: HashMap<String, f64>,                        // member → score，O(1) 分数查询
    sorted: BTreeMap<(OrderedFloat<f64>, String), ()>,   // (score, member) → ()，O(log n) 范围查询
}
```
- `scores`：O(1) ZSCORE 查询
- `sorted`：利用 `BTreeMap` 天然有序性，支持 ZRANGE / ZRANGEBYSCORE / ZRANK 等范围命令

### TTL 实现
每个逻辑数据库维护独立的过期时间表：
```rust
struct LogicalDb {
    data: HashMap<String, RudisObject>,
    expiry: HashMap<String, Instant>,
}
```
- **惰性删除**：每次访问 key 时检查是否过期
- **主动淘汰**：后台定时任务（默认每 100ms）扫描并清理过期键

## 线程安全

使用 `Arc<Mutex<Database>>` 在多连接间共享数据库实例，`parking_lot::Mutex`（非异步锁）确保：
- 锁只在内存操作期间持有（微秒级），不阻塞 Tokio 线程
- 所有连接通过 `Arc::clone` 共享同一个数据库

## RESP 协议

完整实现 RESP（Redis Serialization Protocol）：

| 类型 | 前缀 | 示例 |
|------|------|------|
| Simple String | `+` | `+OK\r\n` |
| Error | `-` | `-ERR message\r\n` |
| Integer | `:` | `:42\r\n` |
| Bulk String | `$` | `$5\r\nhello\r\n` |
| Null | `$-1` | `$-1\r\n` |
| Array | `*` | `*2\r\n...` |

同时支持内联命令格式（如 telnet 直接输入 `PING\r\n`）。

## Lua 脚本

使用 `mlua` crate（Lua 5.4，vendored 编译）实现：
- 每次 EVAL 创建独立的 Lua VM，保证脚本间状态隔离
- 注入 `KEYS`、`ARGV` 全局表（1-indexed，兼容 Redis）
- `redis.call()` / `redis.pcall()` 通过函数回调执行任意 Redis 命令
- `EVALSHA` 通过 SHA1 查找缓存的脚本并执行
- `SCRIPT LOAD` 将脚本加入缓存，返回 SHA1

## 持久化（预留接口）

定义了 `Persistence` trait：
```rust
pub trait Persistence: Send + Sync {
    fn save(&self) -> Result<()>;
    fn load(&self) -> Result<()>;
    fn last_save(&self) -> u64;
}
```

提供两个存根实现：
- `RdbPersistence`：RDB 快照格式，注释中说明了完整实现步骤
- `AofPersistence`：AOF 追加日志格式，支持三种 fsync 策略配置

## 命令覆盖

### String（18 个命令）
SET、GET、GETSET、GETDEL、GETEX、MSET、MGET、MSETNX、SETNX、SETEX、PSETEX、INCR、INCRBY、INCRBYFLOAT、DECR、DECRBY、APPEND、STRLEN、GETRANGE、SETRANGE

### List（14 个命令）
LPUSH、RPUSH、LPUSHX、RPUSHX、LPOP、RPOP、LLEN、LRANGE、LINDEX、LSET、LINSERT、LREM、LTRIM、LMOVE

### Hash（13 个命令）
HSET、HSETNX、HGET、HMGET、HMSET、HDEL、HEXISTS、HGETALL、HKEYS、HVALS、HLEN、HINCRBY、HINCRBYFLOAT

### Set（13 个命令）
SADD、SREM、SMEMBERS、SISMEMBER、SMISMEMBER、SCARD、SUNION、SINTER、SDIFF、SUNIONSTORE、SINTERSTORE、SDIFFSTORE、SPOP、SRANDMEMBER、SMOVE

### ZSet（16 个命令）
ZADD、ZREM、ZSCORE、ZMSCORE、ZINCRBY、ZRANK、ZREVRANK、ZCARD、ZCOUNT、ZRANGE、ZRANGEBYSCORE、ZREVRANGEBYSCORE、ZREVRANGE、ZREMRANGEBYRANK、ZREMRANGEBYSCORE、ZPOPMIN、ZPOPMAX

### 通用（16 个命令）
PING、ECHO、SELECT、DBSIZE、FLUSHDB、FLUSHALL、INFO、DEL、EXISTS、TYPE、RENAME、RENAMENX、EXPIRE、PEXPIRE、EXPIREAT、PEXPIREAT、TTL、PTTL、PERSIST、KEYS、RANDOMKEY、QUIT

### 脚本（4 个命令）
EVAL、EVALSHA、SCRIPT LOAD、SCRIPT FLUSH
