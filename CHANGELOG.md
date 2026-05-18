# Changelog

All notable changes to mini-rudis will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-18

### Added

- **RESP protocol** parser and serializer with full support for Simple String, Error, Integer, Bulk String, Null and Array types; inline command format supported for telnet access
- **String commands**: SET (with EX/PX/EXAT/PXAT/NX/XX/GET/KEEPTTL options), GET, GETSET, GETDEL, GETEX, MSET, MGET, MSETNX, SETNX, SETEX, PSETEX, INCR, INCRBY, INCRBYFLOAT, DECR, DECRBY, APPEND, STRLEN, GETRANGE, SETRANGE
- **List commands**: LPUSH, RPUSH, LPUSHX, RPUSHX, LPOP, RPOP, LLEN, LRANGE, LINDEX, LSET, LINSERT, LREM, LTRIM, LMOVE
- **Hash commands**: HSET, HSETNX, HGET, HMGET, HMSET, HDEL, HEXISTS, HGETALL, HKEYS, HVALS, HLEN, HINCRBY, HINCRBYFLOAT
- **Set commands**: SADD, SREM, SMEMBERS, SISMEMBER, SMISMEMBER, SCARD, SUNION, SINTER, SDIFF, SUNIONSTORE, SINTERSTORE, SDIFFSTORE, SPOP, SRANDMEMBER, SMOVE
- **ZSet commands**: ZADD (with NX/XX/GT/LT/CH flags), ZREM, ZSCORE, ZMSCORE, ZINCRBY, ZRANK, ZREVRANK, ZCARD, ZCOUNT, ZRANGE, ZRANGEBYSCORE, ZREVRANGEBYSCORE, ZREVRANGE, ZREMRANGEBYRANK, ZREMRANGEBYSCORE, ZPOPMIN, ZPOPMAX
- **Generic commands**: DEL, UNLINK, EXISTS, TYPE, RENAME, RENAMENX, EXPIRE, PEXPIRE, EXPIREAT, PEXPIREAT, TTL, PTTL, PERSIST, KEYS, RANDOMKEY, SELECT, DBSIZE, FLUSHDB, FLUSHALL, INFO, PING, ECHO, QUIT
- **Lua scripting**: EVAL, EVALSHA, SCRIPT LOAD, SCRIPT FLUSH; `redis.call()` and `redis.pcall()` support; KEYS/ARGV injection
- **TTL / key expiry**: lazy deletion on access + background active eviction (configurable interval)
- **16 logical databases** with SELECT command
- **RDB and AOF persistence stubs** with documented interface for future implementation
- **Async TCP server** powered by Tokio with per-connection db index isolation
- **CLI** with `--host`, `--port`, `--databases`, `--log-level`, `--eviction-interval-ms` flags
- Comprehensive unit and integration tests (30+ test cases covering all data types and edge conditions)
- GitHub Actions CI pipeline: build, test, clippy, fmt check
