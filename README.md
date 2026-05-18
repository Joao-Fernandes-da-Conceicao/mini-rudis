# mini-rudis

A Redis-compatible in-memory cache server implemented in Rust, supporting the full RESP protocol and five core data structures.

[![CI](https://github.com/YOUR_USERNAME/mini-rudis/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR_USERNAME/mini-rudis/actions/workflows/ci.yml)

## Features

- **Full RESP protocol** — compatible with `redis-cli` and all Redis client libraries
- **Five data types** — String, List, Hash, Set, ZSet (sorted set)
- **TTL / key expiry** — lazy deletion + background active eviction
- **16 logical databases** — `SELECT 0` through `SELECT 15`
- **Lua scripting** — `EVAL`, `EVALSHA`, `SCRIPT` commands with `redis.call()` support
- **Persistence stubs** — RDB and AOF interfaces documented and ready for implementation
- **Async TCP server** — built on Tokio, handles concurrent connections efficiently

## Quick Start

```bash
# Build and run (default: 127.0.0.1:6379)
cargo run --release

# Custom port and log level
cargo run --release -- --port 6380 --log-level debug

# Connect with redis-cli
redis-cli -p 6379 PING
redis-cli -p 6379 SET foo bar
redis-cli -p 6379 GET foo
```

## CLI Options

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `127.0.0.1` | Bind address |
| `--port` / `-p` | `6379` | Listen port |
| `--databases` | `16` | Number of logical databases (max 16) |
| `--log-level` | `info` | Log verbosity (trace/debug/info/warn/error) |
| `--eviction-interval-ms` | `100` | Background expired key eviction interval (ms) |

## Supported Commands

### String
`SET` `GET` `GETSET` `GETDEL` `GETEX` `MSET` `MGET` `MSETNX` `SETNX` `SETEX` `PSETEX`
`INCR` `INCRBY` `INCRBYFLOAT` `DECR` `DECRBY` `APPEND` `STRLEN` `GETRANGE` `SETRANGE`

### List
`LPUSH` `RPUSH` `LPUSHX` `RPUSHX` `LPOP` `RPOP` `LLEN` `LRANGE` `LINDEX` `LSET` `LINSERT` `LREM` `LTRIM` `LMOVE`

### Hash
`HSET` `HSETNX` `HGET` `HMGET` `HMSET` `HDEL` `HEXISTS` `HGETALL` `HKEYS` `HVALS` `HLEN` `HINCRBY` `HINCRBYFLOAT`

### Set
`SADD` `SREM` `SMEMBERS` `SISMEMBER` `SMISMEMBER` `SCARD` `SUNION` `SINTER` `SDIFF`
`SUNIONSTORE` `SINTERSTORE` `SDIFFSTORE` `SPOP` `SRANDMEMBER` `SMOVE`

### Sorted Set (ZSet)
`ZADD` `ZREM` `ZSCORE` `ZMSCORE` `ZINCRBY` `ZRANK` `ZREVRANK` `ZCARD` `ZCOUNT`
`ZRANGE` `ZRANGEBYSCORE` `ZREVRANGEBYSCORE` `ZREVRANGE`
`ZREMRANGEBYRANK` `ZREMRANGEBYSCORE` `ZPOPMIN` `ZPOPMAX`

### Generic
`DEL` `EXISTS` `TYPE` `RENAME` `RENAMENX` `EXPIRE` `PEXPIRE` `EXPIREAT` `PEXPIREAT`
`TTL` `PTTL` `PERSIST` `KEYS` `RANDOMKEY` `SELECT` `DBSIZE` `FLUSHDB` `FLUSHALL`
`INFO` `PING` `ECHO` `QUIT`

### Scripting
`EVAL` `EVALSHA` `SCRIPT LOAD` `SCRIPT FLUSH`

## Lua Scripting Example

```bash
# Basic script with KEYS and ARGV
redis-cli EVAL "redis.call('SET', KEYS[1], ARGV[1]); return redis.call('GET', KEYS[1])" 1 mykey myvalue

# Load and call by SHA1
SHA=$(redis-cli SCRIPT LOAD "return tonumber(ARGV[1]) * 2")
redis-cli EVALSHA $SHA 0 21
```

## Running Tests

```bash
cargo test
```

## Project Structure

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Library root and module exports
├── error.rs         # Unified error types
├── proto/           # RESP protocol parser and serializer
├── db/              # In-memory database (all 5 types + TTL)
├── cmd/             # Command parsing and execution
│   ├── mod.rs       # Command enum, dispatch, helpers
│   ├── string.rs    # String commands
│   ├── list.rs      # List commands
│   ├── hash.rs      # Hash commands
│   ├── set.rs       # Set commands
│   └── zset.rs      # ZSet commands
├── server/          # Async TCP server
│   ├── mod.rs       # Server setup and background tasks
│   └── handler.rs   # Per-connection read/execute/write loop
├── script/          # Lua scripting engine (mlua)
└── persistence/     # RDB/AOF stubs
    ├── mod.rs        # Persistence trait
    ├── rdb.rs        # RDB stub with documented implementation plan
    └── aof.rs        # AOF stub with fsync policy configuration
tests/
├── string_tests.rs  # String integration tests
├── list_tests.rs    # List integration tests
├── hash_tests.rs    # Hash integration tests
├── set_tests.rs     # Set integration tests
├── zset_tests.rs    # ZSet integration tests
└── generic_tests.rs # Key/TTL/generic integration tests
docs/
└── design.md        # Architecture and design documentation
```

## License

MIT
