/// 命令解析与分发层
///
/// 负责将 RESP 帧解码为具体的命令枚举，并调用数据库层执行。
pub mod generic;
pub mod hash;
pub mod list;
pub mod set;
pub mod string;
pub mod zset;

use bytes::Bytes;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::db::Database;
use crate::error::{RedisError, Result};
use crate::proto::Frame;

/// 所有支持的命令枚举（按功能分类）
#[derive(Debug)]
pub enum Command {
    // ── 连接 / 服务器 ──────────────────────────────────────────────
    Ping(Option<Bytes>),
    Echo(Bytes),
    Select(usize),
    Dbsize,
    Flushdb,
    Flushall,
    Info,
    Quit,

    // ── 通用 Key 操作 ──────────────────────────────────────────────
    Del(Vec<String>),
    Exists(Vec<String>),
    Type(String),
    Rename(String, String),
    Renamenx(String, String),
    Expire(String, i64),
    PExpire(String, i64),
    ExpireAt(String, i64),
    PExpireAt(String, i64),
    Ttl(String),
    Pttl(String),
    Persist(String),
    Keys(String),
    Randomkey,

    // ── String ─────────────────────────────────────────────────────
    Set(string::SetArgs),
    Get(String),
    GetSet(String, Bytes),
    GetDel(String),
    GetEx(string::GetExArgs),
    Mset(Vec<(String, Bytes)>),
    Mget(Vec<String>),
    Msetnx(Vec<(String, Bytes)>),
    Setnx(String, Bytes),
    Setex(String, i64, Bytes),
    PSetex(String, i64, Bytes),
    Incr(String),
    IncrBy(String, i64),
    IncrByFloat(String, f64),
    Decr(String),
    DecrBy(String, i64),
    Append(String, Bytes),
    Strlen(String),
    GetRange(String, i64, i64),
    SetRange(String, usize, Bytes),

    // ── List ───────────────────────────────────────────────────────
    LPush(String, Vec<Bytes>),
    RPush(String, Vec<Bytes>),
    LPushX(String, Vec<Bytes>),
    RPushX(String, Vec<Bytes>),
    LPop(String, usize),
    RPop(String, usize),
    LLen(String),
    LRange(String, i64, i64),
    LIndex(String, i64),
    LSet(String, i64, Bytes),
    LInsert(String, bool, Bytes, Bytes), // bool: true=BEFORE
    LRem(String, i64, Bytes),
    LTrim(String, i64, i64),
    LMove(String, String, bool, bool), // src, dst, src_left, dst_left

    // ── Hash ───────────────────────────────────────────────────────
    HSet(String, Vec<(String, Bytes)>),
    HSetnx(String, String, Bytes),
    HGet(String, String),
    HMGet(String, Vec<String>),
    HMSet(String, Vec<(String, Bytes)>),
    HDel(String, Vec<String>),
    HExists(String, String),
    HGetAll(String),
    HKeys(String),
    HVals(String),
    HLen(String),
    HIncrBy(String, String, i64),
    HIncrByFloat(String, String, f64),

    // ── Set ────────────────────────────────────────────────────────
    SAdd(String, Vec<String>),
    SRem(String, Vec<String>),
    SMembers(String),
    SIsMember(String, String),
    SMIsMember(String, Vec<String>),
    SCard(String),
    SUnion(Vec<String>),
    SInter(Vec<String>),
    SDiff(Vec<String>),
    SUnionStore(String, Vec<String>),
    SInterStore(String, Vec<String>),
    SDiffStore(String, Vec<String>),
    SPop(String, usize),
    SRandMember(String, i64),
    SMove(String, String, String),

    // ── ZSet ───────────────────────────────────────────────────────
    ZAdd(zset::ZAddArgs),
    ZRem(String, Vec<String>),
    ZScore(String, String),
    ZMScore(String, Vec<String>),
    ZIncrBy(String, f64, String),
    ZRank(String, String),
    ZRevRank(String, String),
    ZCard(String),
    ZCount(String, String, String),
    ZRange(String, i64, i64, bool, bool), // key, start, stop, rev, withscores
    ZRangeByScore(zset::ZRangeByScoreArgs),
    ZRevRangeByScore(zset::ZRangeByScoreArgs),
    ZRevRange(String, i64, i64, bool),
    ZRemRangeByRank(String, i64, i64),
    ZRemRangeByScore(String, String, String),
    ZPopMin(String, usize),
    ZPopMax(String, usize),

    // ── Scripting ─────────────────────────────────────────────────
    Eval(Bytes, usize, Vec<Bytes>, Vec<Bytes>),
    EvalSha(Bytes, usize, Vec<Bytes>, Vec<Bytes>),
    ScriptLoad(Bytes),
    ScriptFlush,
}

/// 从 RESP Bulk 参数列表解析为 Command
pub fn parse(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::Protocol("empty command".into()));
    }
    let cmd_name = String::from_utf8_lossy(&args[0]).to_uppercase();
    let rest = &args[1..];

    match cmd_name.as_str() {
        // ── 服务器 ───────────────────────────────────────────────────
        "PING" => generic::parse_ping(rest),
        "ECHO" => generic::parse_echo(rest),
        "SELECT" => generic::parse_select(rest),
        "DBSIZE" => Ok(Command::Dbsize),
        "FLUSHDB" => Ok(Command::Flushdb),
        "FLUSHALL" => Ok(Command::Flushall),
        "INFO" => Ok(Command::Info),
        "QUIT" => Ok(Command::Quit),

        // ── 通用 Key ────────────────────────────────────────────────
        "DEL" | "UNLINK" => generic::parse_del(rest),
        "EXISTS" => generic::parse_exists(rest),
        "TYPE" => generic::parse_type(rest),
        "RENAME" => generic::parse_rename(rest, false),
        "RENAMENX" => generic::parse_rename(rest, true),
        "EXPIRE" => generic::parse_expire(rest, false, false),
        "PEXPIRE" => generic::parse_pexpire(rest),
        "EXPIREAT" => generic::parse_expire(rest, true, false),
        "PEXPIREAT" => generic::parse_pexpireat(rest),
        "TTL" => generic::parse_ttl(rest, false),
        "PTTL" => generic::parse_ttl(rest, true),
        "PERSIST" => generic::parse_persist(rest),
        "KEYS" => generic::parse_keys(rest),
        "RANDOMKEY" => Ok(Command::Randomkey),

        // ── String ──────────────────────────────────────────────────
        "SET" => string::parse_set(rest),
        "GET" => string::parse_get(rest),
        "GETSET" => string::parse_getset(rest),
        "GETDEL" => string::parse_getdel(rest),
        "GETEX" => string::parse_getex(rest),
        "MSET" => string::parse_mset(rest),
        "MGET" => string::parse_mget(rest),
        "MSETNX" => string::parse_msetnx(rest),
        "SETNX" => string::parse_setnx(rest),
        "SETEX" => string::parse_setex(rest),
        "PSETEX" => string::parse_psetex(rest),
        "INCR" => string::parse_incr(rest, "INCR"),
        "INCRBY" => string::parse_incrby(rest),
        "INCRBYFLOAT" => string::parse_incrbyfloat(rest),
        "DECR" => string::parse_decr(rest),
        "DECRBY" => string::parse_decrby(rest),
        "APPEND" => string::parse_append(rest),
        "STRLEN" => string::parse_strlen(rest),
        "GETRANGE" | "SUBSTR" => string::parse_getrange(rest),
        "SETRANGE" => string::parse_setrange(rest),

        // ── List ────────────────────────────────────────────────────
        "LPUSH" => list::parse_push(rest, true, false),
        "RPUSH" => list::parse_push(rest, false, false),
        "LPUSHX" => list::parse_push(rest, true, true),
        "RPUSHX" => list::parse_push(rest, false, true),
        "LPOP" => list::parse_pop(rest, true),
        "RPOP" => list::parse_pop(rest, false),
        "LLEN" => list::parse_llen(rest),
        "LRANGE" => list::parse_lrange(rest),
        "LINDEX" => list::parse_lindex(rest),
        "LSET" => list::parse_lset(rest),
        "LINSERT" => list::parse_linsert(rest),
        "LREM" => list::parse_lrem(rest),
        "LTRIM" => list::parse_ltrim(rest),
        "LMOVE" => list::parse_lmove(rest),

        // ── Hash ────────────────────────────────────────────────────
        "HSET" => hash::parse_hset(rest),
        "HSETNX" => hash::parse_hsetnx(rest),
        "HGET" => hash::parse_hget(rest),
        "HMGET" => hash::parse_hmget(rest),
        "HMSET" => hash::parse_hmset(rest),
        "HDEL" => hash::parse_hdel(rest),
        "HEXISTS" => hash::parse_hexists(rest),
        "HGETALL" => hash::parse_hgetall(rest),
        "HKEYS" => hash::parse_hkeys(rest),
        "HVALS" => hash::parse_hvals(rest),
        "HLEN" => hash::parse_hlen(rest),
        "HINCRBY" => hash::parse_hincrby(rest),
        "HINCRBYFLOAT" => hash::parse_hincrbyfloat(rest),

        // ── Set ─────────────────────────────────────────────────────
        "SADD" => set::parse_sadd(rest),
        "SREM" => set::parse_srem(rest),
        "SMEMBERS" => set::parse_smembers(rest),
        "SISMEMBER" => set::parse_sismember(rest),
        "SMISMEMBER" => set::parse_smismember(rest),
        "SCARD" => set::parse_scard(rest),
        "SUNION" => set::parse_sunion(rest),
        "SINTER" => set::parse_sinter(rest),
        "SDIFF" => set::parse_sdiff(rest),
        "SUNIONSTORE" => set::parse_store(rest, "SUNIONSTORE"),
        "SINTERSTORE" => set::parse_store(rest, "SINTERSTORE"),
        "SDIFFSTORE" => set::parse_store(rest, "SDIFFSTORE"),
        "SPOP" => set::parse_spop(rest),
        "SRANDMEMBER" => set::parse_srandmember(rest),
        "SMOVE" => set::parse_smove(rest),

        // ── ZSet ────────────────────────────────────────────────────
        "ZADD" => zset::parse_zadd(rest),
        "ZREM" => zset::parse_zrem(rest),
        "ZSCORE" => zset::parse_zscore(rest),
        "ZMSCORE" => zset::parse_zmscore(rest),
        "ZINCRBY" => zset::parse_zincrby(rest),
        "ZRANK" => zset::parse_zrank(rest),
        "ZREVRANK" => zset::parse_zrevrank(rest),
        "ZCARD" => zset::parse_zcard(rest),
        "ZCOUNT" => zset::parse_zcount(rest),
        "ZRANGE" => zset::parse_zrange(rest),
        "ZRANGEBYSCORE" => zset::parse_zrangebyscore(rest, false),
        "ZREVRANGEBYSCORE" => zset::parse_zrangebyscore(rest, true),
        "ZREVRANGE" => zset::parse_zrevrange(rest),
        "ZREMRANGEBYRANK" => zset::parse_zremrangebyrank(rest),
        "ZREMRANGEBYSCORE" => zset::parse_zremrangebyscore(rest),
        "ZPOPMIN" => zset::parse_zpop(rest, true),
        "ZPOPMAX" => zset::parse_zpop(rest, false),

        // ── Scripting ───────────────────────────────────────────────
        "EVAL" => parse_eval(rest, false),
        "EVALSHA" => parse_eval(rest, true),
        "SCRIPT" => parse_script(rest),

        _ => Err(RedisError::Generic(format!(
            "ERR unknown command `{cmd_name}`, with args beginning with: {}",
            rest.iter()
                .take(3)
                .map(|b| format!("`{}`", String::from_utf8_lossy(b)))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn parse_eval(args: &[Bytes], is_sha: bool) -> Result<Command> {
    if args.len() < 2 {
        let name = if is_sha { "EVALSHA" } else { "EVAL" };
        return Err(RedisError::WrongArity(name.into()));
    }
    let script = args[0].clone();
    let numkeys: usize = parse_usize(&args[1], "EVAL")?;
    if args.len() < 2 + numkeys {
        return Err(RedisError::Generic(
            "ERR not enough arguments for EVAL".into(),
        ));
    }
    let keys = args[2..2 + numkeys].to_vec();
    let argv = args[2 + numkeys..].to_vec();
    if is_sha {
        Ok(Command::EvalSha(script, numkeys, keys, argv))
    } else {
        Ok(Command::Eval(script, numkeys, keys, argv))
    }
}

fn parse_script(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("SCRIPT".into()));
    }
    let sub = String::from_utf8_lossy(&args[0]).to_uppercase();
    match sub.as_str() {
        "LOAD" => {
            if args.len() < 2 {
                return Err(RedisError::WrongArity("SCRIPT LOAD".into()));
            }
            Ok(Command::ScriptLoad(args[1].clone()))
        }
        "FLUSH" => Ok(Command::ScriptFlush),
        _ => Err(RedisError::Generic(format!(
            "ERR unknown SCRIPT subcommand: {sub}"
        ))),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  执行命令
// ─────────────────────────────────────────────────────────────────────────────

/// 执行命令，返回 RESP 响应帧。
/// `db_index` 是当前连接选择的逻辑库编号。
pub fn execute(
    cmd: Command,
    db: &Arc<Mutex<Database>>,
    db_index: &mut usize,
    script_engine: &Arc<crate::script::ScriptEngine>,
) -> Frame {
    match cmd {
        Command::Ping(msg) => match msg {
            None => Frame::pong(),
            Some(m) => Frame::Bulk(m),
        },
        Command::Echo(msg) => Frame::Bulk(msg),
        Command::Select(idx) => {
            if idx >= 16 {
                return Frame::from_error(&RedisError::DbIndexOutOfRange);
            }
            *db_index = idx;
            Frame::ok()
        }
        Command::Dbsize => {
            let db = db.lock();
            match db.dbsize(*db_index) {
                Ok(n) => Frame::int(n as i64),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::Flushdb => {
            let mut db = db.lock();
            match db.flushdb(*db_index) {
                Ok(_) => Frame::ok(),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::Flushall => {
            db.lock().flushall();
            Frame::ok()
        }
        Command::Info => Frame::bulk_str(info_string()),
        Command::Quit => Frame::ok(),

        // ── 通用 Key ────────────────────────────────────────────────
        Command::Del(keys) => int_result(db.lock().del(*db_index, &keys)),
        Command::Exists(keys) => int_result(db.lock().exists(*db_index, &keys)),
        Command::Type(key) => match db.lock().type_of(*db_index, &key) {
            Ok(t) => Frame::Simple(t.to_string()),
            Err(e) => Frame::from_error(&e),
        },
        Command::Rename(src, dst) => ok_result(db.lock().rename(*db_index, &src, &dst)),
        Command::Renamenx(src, dst) => bool_int_result(db.lock().renamenx(*db_index, &src, &dst)),
        Command::Expire(key, secs) => bool_int_result(db.lock().expire(*db_index, &key, secs)),
        Command::PExpire(key, ms) => bool_int_result(db.lock().pexpire(*db_index, &key, ms)),
        Command::ExpireAt(key, ts) => bool_int_result(db.lock().expireat(*db_index, &key, ts)),
        Command::PExpireAt(key, ts) => bool_int_result(db.lock().pexpireat(*db_index, &key, ts)),
        Command::Ttl(key) => int_result(db.lock().ttl(*db_index, &key)),
        Command::Pttl(key) => int_result(db.lock().pttl(*db_index, &key)),
        Command::Persist(key) => bool_int_result(db.lock().persist(*db_index, &key)),
        Command::Keys(pattern) => match db.lock().keys(*db_index, &pattern) {
            Ok(keys) => Frame::Array(keys.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::Randomkey => match db.lock().randomkey(*db_index) {
            Ok(Some(k)) => Frame::bulk_str(k),
            Ok(None) => Frame::Null,
            Err(e) => Frame::from_error(&e),
        },

        // ── String ──────────────────────────────────────────────────
        cmd => string::execute(cmd, db, db_index, script_engine),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  辅助返回值构造
// ─────────────────────────────────────────────────────────────────────────────

pub fn int_result(r: Result<i64>) -> Frame {
    match r {
        Ok(n) => Frame::int(n),
        Err(e) => Frame::from_error(&e),
    }
}

pub fn ok_result(r: Result<()>) -> Frame {
    match r {
        Ok(_) => Frame::ok(),
        Err(e) => Frame::from_error(&e),
    }
}

pub fn bool_int_result(r: Result<bool>) -> Frame {
    match r {
        Ok(b) => Frame::int(if b { 1 } else { 0 }),
        Err(e) => Frame::from_error(&e),
    }
}

pub fn null_or_bulk(r: Result<Option<Vec<u8>>>) -> Frame {
    match r {
        Ok(Some(v)) => Frame::bulk_bytes(v),
        Ok(None) => Frame::Null,
        Err(e) => Frame::from_error(&e),
    }
}

pub fn bulk_array(r: Result<Vec<Vec<u8>>>) -> Frame {
    match r {
        Ok(items) => {
            if items.is_empty() {
                Frame::Array(vec![])
            } else {
                Frame::Array(items.into_iter().map(Frame::bulk_bytes).collect())
            }
        }
        Err(e) => Frame::from_error(&e),
    }
}

pub fn str_array(r: Result<Vec<String>>) -> Frame {
    match r {
        Ok(items) => Frame::Array(items.into_iter().map(Frame::bulk_str).collect()),
        Err(e) => Frame::from_error(&e),
    }
}

pub fn parse_i64(b: &Bytes, cmd: &str) -> Result<i64> {
    let s = std::str::from_utf8(b).map_err(|_| RedisError::WrongArity(cmd.into()))?;
    s.trim().parse::<i64>().map_err(|_| RedisError::NotInteger)
}

pub fn parse_usize(b: &Bytes, cmd: &str) -> Result<usize> {
    let n = parse_i64(b, cmd)?;
    if n < 0 {
        return Err(RedisError::Generic(
            "ERR value is out of range, must be a positive integer or zero".into(),
        ));
    }
    Ok(n as usize)
}

pub fn parse_f64(b: &Bytes, _cmd: &str) -> Result<f64> {
    let s = std::str::from_utf8(b).map_err(|_| RedisError::NotFloat)?;
    s.trim().parse::<f64>().map_err(|_| RedisError::NotFloat)
}

fn info_string() -> String {
    format!(
        "# Server\r\nredis_version:7.0.0-mini-rudis\r\nredis_mode:standalone\r\nos:{}\r\n\
         arch_bits:64\r\nexecutable:mini-rudis\r\n\
         # Keyspace\r\n",
        std::env::consts::OS,
    )
}
