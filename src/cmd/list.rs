use bytes::Bytes;
use std::rc::Rc;

use crate::cmd::{bulk_array, int_result, ok_result, parse_i64, parse_usize, Command};
use crate::error::{RedisError, Result};
use crate::proto::Frame;
use crate::SharedDb;

pub fn parse_push(args: &[Bytes], left: bool, x: bool) -> Result<Command> {
    if args.len() < 2 {
        let name = match (left, x) {
            (true, false) => "LPUSH",
            (false, false) => "RPUSH",
            (true, true) => "LPUSHX",
            (false, true) => "RPUSHX",
        };
        return Err(RedisError::WrongArity(name.into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let values: Vec<Bytes> = args[1..].to_vec();
    match (left, x) {
        (true, false) => Ok(Command::LPush(key, values)),
        (false, false) => Ok(Command::RPush(key, values)),
        (true, true) => Ok(Command::LPushX(key, values)),
        (false, true) => Ok(Command::RPushX(key, values)),
    }
}

pub fn parse_pop(args: &[Bytes], left: bool) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity(
            if left { "LPOP" } else { "RPOP" }.into(),
        ));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let count = if args.len() >= 2 {
        parse_usize(&args[1], if left { "LPOP" } else { "RPOP" })?
    } else {
        1
    };
    if left {
        Ok(Command::LPop(key, count))
    } else {
        Ok(Command::RPop(key, count))
    }
}

pub fn parse_llen(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("LLEN".into()));
    }
    Ok(Command::LLen(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_lrange(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("LRANGE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let start = parse_i64(&args[1], "LRANGE")?;
    let stop = parse_i64(&args[2], "LRANGE")?;
    Ok(Command::LRange(key, start, stop))
}

pub fn parse_lindex(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("LINDEX".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let index = parse_i64(&args[1], "LINDEX")?;
    Ok(Command::LIndex(key, index))
}

pub fn parse_lset(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("LSET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let index = parse_i64(&args[1], "LSET")?;
    Ok(Command::LSet(key, index, args[2].clone()))
}

pub fn parse_linsert(args: &[Bytes]) -> Result<Command> {
    if args.len() != 4 {
        return Err(RedisError::WrongArity("LINSERT".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let pos = String::from_utf8_lossy(&args[1]).to_uppercase();
    let before = match pos.as_str() {
        "BEFORE" => true,
        "AFTER" => false,
        _ => return Err(RedisError::SyntaxError),
    };
    Ok(Command::LInsert(
        key,
        before,
        args[2].clone(),
        args[3].clone(),
    ))
}

pub fn parse_lrem(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("LREM".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let count = parse_i64(&args[1], "LREM")?;
    Ok(Command::LRem(key, count, args[2].clone()))
}

pub fn parse_ltrim(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("LTRIM".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let start = parse_i64(&args[1], "LTRIM")?;
    let stop = parse_i64(&args[2], "LTRIM")?;
    Ok(Command::LTrim(key, start, stop))
}

pub fn parse_lmove(args: &[Bytes]) -> Result<Command> {
    if args.len() != 4 {
        return Err(RedisError::WrongArity("LMOVE".into()));
    }
    let src = String::from_utf8_lossy(&args[0]).into_owned();
    let dst = String::from_utf8_lossy(&args[1]).into_owned();
    let src_dir = String::from_utf8_lossy(&args[2]).to_uppercase();
    let dst_dir = String::from_utf8_lossy(&args[3]).to_uppercase();
    let src_left = match src_dir.as_str() {
        "LEFT" => true,
        "RIGHT" => false,
        _ => return Err(RedisError::SyntaxError),
    };
    let dst_left = match dst_dir.as_str() {
        "LEFT" => true,
        "RIGHT" => false,
        _ => return Err(RedisError::SyntaxError),
    };
    Ok(Command::LMove(src, dst, src_left, dst_left))
}

pub fn execute(
    cmd: Command,
    db: &SharedDb,
    db_index: &mut usize,
    script_engine: &Rc<crate::script::ScriptEngine>,
) -> Frame {
    match cmd {
        Command::LPush(key, vals) => {
            let vals: Vec<Vec<u8>> = vals.into_iter().map(|b| b.to_vec()).collect();
            int_result(db.borrow_mut().lpush(*db_index, &key, vals).map(|n| n as i64))
        }
        Command::RPush(key, vals) => {
            let vals: Vec<Vec<u8>> = vals.into_iter().map(|b| b.to_vec()).collect();
            int_result(db.borrow_mut().rpush(*db_index, &key, vals).map(|n| n as i64))
        }
        Command::LPushX(key, vals) => {
            let vals: Vec<Vec<u8>> = vals.into_iter().map(|b| b.to_vec()).collect();
            int_result(db.borrow_mut().lpushx(*db_index, &key, vals).map(|n| n as i64))
        }
        Command::RPushX(key, vals) => {
            let vals: Vec<Vec<u8>> = vals.into_iter().map(|b| b.to_vec()).collect();
            int_result(db.borrow_mut().rpushx(*db_index, &key, vals).map(|n| n as i64))
        }
        Command::LPop(key, count) => {
            let mut borrowed = db.borrow_mut();
            let result = borrowed.lpop(*db_index, &key, count);
            drop(borrowed);
            exec_pop_result(result, count == 1)
        }
        Command::RPop(key, count) => {
            let mut borrowed = db.borrow_mut();
            let result = borrowed.rpop(*db_index, &key, count);
            drop(borrowed);
            exec_pop_result(result, count == 1)
        }
        Command::LLen(key) => int_result(db.borrow_mut().llen(*db_index, &key).map(|n| n as i64)),
        Command::LRange(key, start, stop) => {
            bulk_array(db.borrow_mut().lrange(*db_index, &key, start, stop))
        }
        Command::LIndex(key, index) => match db.borrow_mut().lindex(*db_index, &key, index) {
            Ok(Some(v)) => Frame::bulk_bytes(v),
            Ok(None) => Frame::Null,
            Err(e) => Frame::from_error(&e),
        },
        Command::LSet(key, index, val) => {
            ok_result(db.borrow_mut().lset(*db_index, &key, index, val.to_vec()))
        }
        Command::LInsert(key, before, pivot, value) => {
            int_result(
                db.borrow_mut()
                    .linsert(*db_index, &key, before, &pivot, value.to_vec()),
            )
        }
        Command::LRem(key, count, val) => int_result(
            db.borrow_mut()
                .lrem(*db_index, &key, count, &val)
                .map(|n| n as i64),
        ),
        Command::LTrim(key, start, stop) => {
            ok_result(db.borrow_mut().ltrim(*db_index, &key, start, stop))
        }
        Command::LMove(src, dst, src_left, dst_left) => {
            match db.borrow_mut().lmove(*db_index, &src, &dst, src_left, dst_left) {
                Ok(Some(v)) => Frame::bulk_bytes(v),
                Ok(None) => Frame::Null,
                Err(e) => Frame::from_error(&e),
            }
        }

        cmd => crate::cmd::hash::execute(cmd, db, db_index, script_engine),
    }
}

/// 统一处理 LPOP/RPOP 的返回格式：
/// - 如果请求 count=1（不带 count 参数），返回单个 Bulk 或 Null
/// - 否则返回数组
fn exec_pop_result(result: crate::error::Result<Vec<Vec<u8>>>, single: bool) -> Frame {
    match result {
        Err(e) => Frame::from_error(&e),
        Ok(items) if items.is_empty() => Frame::Null,
        Ok(items) if single => Frame::bulk_bytes(items.into_iter().next().unwrap()),
        Ok(items) => Frame::Array(items.into_iter().map(Frame::bulk_bytes).collect()),
    }
}
