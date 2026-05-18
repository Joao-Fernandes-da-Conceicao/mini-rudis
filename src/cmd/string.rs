use bytes::Bytes;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

use crate::cmd::{
    bool_int_result, int_result, null_or_bulk, ok_result, parse_f64, parse_i64, parse_usize,
    Command,
};
use crate::db::{format_float, Database};
use crate::error::{RedisError, Result};
use crate::proto::Frame;

// ─────────────────────────────────────────────────────────────────────────────
//  SET 命令参数结构
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SetArgs {
    pub key: String,
    pub value: Bytes,
    pub expire: Option<Duration>,
    pub nx: bool,
    pub xx: bool,
    pub get: bool,
    pub keepttl: bool,
}

#[derive(Debug)]
pub struct GetExArgs {
    pub key: String,
    pub expire: Option<Duration>,
    pub persist: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
//  解析函数
// ─────────────────────────────────────────────────────────────────────────────

pub fn parse_set(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("SET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let value = args[1].clone();

    let mut expire: Option<Duration> = None;
    let mut nx = false;
    let mut xx = false;
    let mut get = false;
    let mut keepttl = false;

    let mut i = 2;
    while i < args.len() {
        let opt = String::from_utf8_lossy(&args[i]).to_uppercase();
        match opt.as_str() {
            "EX" => {
                i += 1;
                let secs = parse_i64(&args[i], "SET")?;
                if secs <= 0 {
                    return Err(RedisError::Generic(
                        "ERR invalid expire time in 'set' command".into(),
                    ));
                }
                expire = Some(Duration::from_secs(secs as u64));
            }
            "PX" => {
                i += 1;
                let ms = parse_i64(&args[i], "SET")?;
                if ms <= 0 {
                    return Err(RedisError::Generic(
                        "ERR invalid expire time in 'set' command".into(),
                    ));
                }
                expire = Some(Duration::from_millis(ms as u64));
            }
            "EXAT" => {
                i += 1;
                let ts = parse_i64(&args[i], "SET")?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs() as i64;
                let diff = (ts - now).max(0) as u64;
                expire = Some(Duration::from_secs(diff));
            }
            "PXAT" => {
                i += 1;
                let ts = parse_i64(&args[i], "SET")?;
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_millis() as i64;
                let diff = (ts - now_ms).max(0) as u64;
                expire = Some(Duration::from_millis(diff));
            }
            "NX" => nx = true,
            "XX" => xx = true,
            "GET" => get = true,
            "KEEPTTL" => keepttl = true,
            _ => return Err(RedisError::SyntaxError),
        }
        i += 1;
    }

    if nx && xx {
        return Err(RedisError::SyntaxError);
    }

    Ok(Command::Set(SetArgs {
        key,
        value,
        expire,
        nx,
        xx,
        get,
        keepttl,
    }))
}

pub fn parse_get(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("GET".into()));
    }
    Ok(Command::Get(String::from_utf8_lossy(&args[0]).into_owned()))
}

pub fn parse_getset(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("GETSET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    Ok(Command::GetSet(key, args[1].clone()))
}

pub fn parse_getdel(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("GETDEL".into()));
    }
    Ok(Command::GetDel(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_getex(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("GETEX".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let mut expire: Option<Duration> = None;
    let mut persist = false;
    let mut i = 1;
    while i < args.len() {
        let opt = String::from_utf8_lossy(&args[i]).to_uppercase();
        match opt.as_str() {
            "EX" => {
                i += 1;
                let s = parse_i64(&args[i], "GETEX")?;
                expire = Some(Duration::from_secs(s as u64));
            }
            "PX" => {
                i += 1;
                let ms = parse_i64(&args[i], "GETEX")?;
                expire = Some(Duration::from_millis(ms as u64));
            }
            "PERSIST" => persist = true,
            _ => return Err(RedisError::SyntaxError),
        }
        i += 1;
    }
    Ok(Command::GetEx(GetExArgs {
        key,
        expire,
        persist,
    }))
}

pub fn parse_mset(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return Err(RedisError::WrongArity("MSET".into()));
    }
    let pairs = args
        .chunks(2)
        .map(|c| (String::from_utf8_lossy(&c[0]).into_owned(), c[1].clone()))
        .collect();
    Ok(Command::Mset(pairs))
}

pub fn parse_mget(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("MGET".into()));
    }
    Ok(Command::Mget(
        args.iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect(),
    ))
}

pub fn parse_msetnx(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return Err(RedisError::WrongArity("MSETNX".into()));
    }
    let pairs = args
        .chunks(2)
        .map(|c| (String::from_utf8_lossy(&c[0]).into_owned(), c[1].clone()))
        .collect();
    Ok(Command::Msetnx(pairs))
}

pub fn parse_setnx(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("SETNX".into()));
    }
    Ok(Command::Setnx(
        String::from_utf8_lossy(&args[0]).into_owned(),
        args[1].clone(),
    ))
}

pub fn parse_setex(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("SETEX".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let secs = parse_i64(&args[1], "SETEX")?;
    if secs <= 0 {
        return Err(RedisError::Generic(
            "ERR invalid expire time in 'setex' command".into(),
        ));
    }
    Ok(Command::Setex(key, secs, args[2].clone()))
}

pub fn parse_psetex(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("PSETEX".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let ms = parse_i64(&args[1], "PSETEX")?;
    if ms <= 0 {
        return Err(RedisError::Generic(
            "ERR invalid expire time in 'psetex' command".into(),
        ));
    }
    Ok(Command::PSetex(key, ms, args[2].clone()))
}

pub fn parse_incr(args: &[Bytes], cmd: &str) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity(cmd.into()));
    }
    Ok(Command::Incr(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_incrby(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("INCRBY".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let delta = parse_i64(&args[1], "INCRBY")?;
    Ok(Command::IncrBy(key, delta))
}

pub fn parse_incrbyfloat(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("INCRBYFLOAT".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let delta = parse_f64(&args[1], "INCRBYFLOAT")?;
    Ok(Command::IncrByFloat(key, delta))
}

pub fn parse_decr(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("DECR".into()));
    }
    Ok(Command::Decr(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_decrby(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("DECRBY".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let delta = parse_i64(&args[1], "DECRBY")?;
    Ok(Command::DecrBy(key, delta))
}

pub fn parse_append(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("APPEND".into()));
    }
    Ok(Command::Append(
        String::from_utf8_lossy(&args[0]).into_owned(),
        args[1].clone(),
    ))
}

pub fn parse_strlen(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("STRLEN".into()));
    }
    Ok(Command::Strlen(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_getrange(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("GETRANGE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let start = parse_i64(&args[1], "GETRANGE")?;
    let end = parse_i64(&args[2], "GETRANGE")?;
    Ok(Command::GetRange(key, start, end))
}

pub fn parse_setrange(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("SETRANGE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let offset = parse_usize(&args[1], "SETRANGE")?;
    Ok(Command::SetRange(key, offset, args[2].clone()))
}

// ─────────────────────────────────────────────────────────────────────────────
//  执行函数（被 cmd/mod.rs 中的 execute 分发到这里）
// ─────────────────────────────────────────────────────────────────────────────

pub fn execute(
    cmd: Command,
    db: &Arc<Mutex<Database>>,
    db_index: &mut usize,
    script_engine: &Arc<crate::script::ScriptEngine>,
) -> Frame {
    match cmd {
        Command::Set(args) => exec_set(args, db, *db_index),
        Command::Get(key) => null_or_bulk(db.lock().str_get(*db_index, &key)),
        Command::GetSet(key, val) => {
            null_or_bulk(db.lock().str_getset(*db_index, key, val.to_vec()))
        }
        Command::GetDel(key) => null_or_bulk(db.lock().str_getdel(*db_index, &key)),
        Command::GetEx(args) => exec_getex(args, db, *db_index),
        Command::Mset(pairs) => {
            let pairs: Vec<(String, Vec<u8>)> =
                pairs.into_iter().map(|(k, v)| (k, v.to_vec())).collect();
            ok_result(db.lock().mset(*db_index, pairs))
        }
        Command::Mget(keys) => match db.lock().mget(*db_index, &keys) {
            Ok(values) => Frame::Array(
                values
                    .into_iter()
                    .map(|v| match v {
                        Some(b) => Frame::bulk_bytes(b),
                        None => Frame::Null,
                    })
                    .collect(),
            ),
            Err(e) => Frame::from_error(&e),
        },
        Command::Msetnx(pairs) => {
            let pairs: Vec<(String, Vec<u8>)> =
                pairs.into_iter().map(|(k, v)| (k, v.to_vec())).collect();
            bool_int_result(db.lock().msetnx(*db_index, pairs))
        }
        Command::Setnx(key, val) => {
            bool_int_result(db.lock().str_setnx(*db_index, key, val.to_vec()))
        }
        Command::Setex(key, secs, val) => ok_result(db.lock().str_set(
            *db_index,
            key,
            val.to_vec(),
            Some(Duration::from_secs(secs as u64)),
        )),
        Command::PSetex(key, ms, val) => ok_result(db.lock().str_set(
            *db_index,
            key,
            val.to_vec(),
            Some(Duration::from_millis(ms as u64)),
        )),
        Command::Incr(key) => int_result(db.lock().str_incr(*db_index, &key, 1)),
        Command::IncrBy(key, delta) => int_result(db.lock().str_incr(*db_index, &key, delta)),
        Command::IncrByFloat(key, delta) => {
            match db.lock().str_incr_float(*db_index, &key, delta) {
                Ok(v) => Frame::bulk_str(format_float(v)),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::Decr(key) => int_result(db.lock().str_incr(*db_index, &key, -1)),
        Command::DecrBy(key, delta) => int_result(db.lock().str_incr(*db_index, &key, -delta)),
        Command::Append(key, val) => int_result(
            db.lock()
                .str_append(*db_index, &key, &val)
                .map(|n| n as i64),
        ),
        Command::Strlen(key) => int_result(db.lock().str_strlen(*db_index, &key).map(|n| n as i64)),
        Command::GetRange(key, start, end) => {
            match db.lock().str_getrange(*db_index, &key, start, end) {
                Ok(v) => Frame::bulk_bytes(v),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::SetRange(key, offset, val) => int_result(
            db.lock()
                .str_setrange(*db_index, &key, offset, &val)
                .map(|n| n as i64),
        ),

        // ── 分发到下层模块 ──────────────────────────────────────────
        cmd => crate::cmd::list::execute(cmd, db, db_index, script_engine),
    }
}

fn exec_set(args: SetArgs, db: &Arc<Mutex<Database>>, db_index: usize) -> Frame {
    let mut locked = db.lock();
    let old_val = if args.get {
        match locked.str_get(db_index, &args.key) {
            Ok(v) => v,
            Err(e) => return Frame::from_error(&e),
        }
    } else {
        None
    };

    let result = if args.nx {
        match locked.str_setnx(db_index, args.key.clone(), args.value.to_vec()) {
            Ok(true) => {
                if let Some(exp) = args.expire {
                    let _ = locked.expire(db_index, &args.key, exp.as_secs() as i64);
                }
                Ok(true)
            }
            other => other,
        }
    } else if args.xx {
        locked.str_setxx(db_index, args.key, args.value.to_vec(), args.expire)
    } else {
        locked
            .str_set(db_index, args.key, args.value.to_vec(), args.expire)
            .map(|_| true)
    };

    match result {
        Ok(true) => {
            if args.get {
                match old_val {
                    Some(v) => Frame::bulk_bytes(v),
                    None => Frame::Null,
                }
            } else {
                Frame::ok()
            }
        }
        Ok(false) => Frame::Null,
        Err(e) => Frame::from_error(&e),
    }
}

fn exec_getex(args: GetExArgs, db: &Arc<Mutex<Database>>, db_index: usize) -> Frame {
    let mut locked = db.lock();
    let val = match locked.str_get(db_index, &args.key) {
        Ok(v) => v,
        Err(e) => return Frame::from_error(&e),
    };
    if val.is_none() {
        return Frame::Null;
    }
    if args.persist {
        let _ = locked.persist(db_index, &args.key);
    } else if let Some(exp) = args.expire {
        let _ = locked.pexpire(db_index, &args.key, exp.as_millis() as i64);
    }
    Frame::bulk_bytes(val.unwrap())
}
