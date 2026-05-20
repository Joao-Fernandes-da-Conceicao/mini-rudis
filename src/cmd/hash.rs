use bytes::Bytes;
use std::rc::Rc;

use crate::cmd::{bool_int_result, int_result, ok_result, parse_f64, parse_i64, Command};
use crate::db::format_float;
use crate::error::{RedisError, Result};
use crate::proto::Frame;
use crate::SharedDb;

pub fn parse_hset(args: &[Bytes]) -> Result<Command> {
    if args.len() < 3 || args.len().is_multiple_of(2) {
        return Err(RedisError::WrongArity("HSET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let fields: Vec<(String, Bytes)> = args[1..]
        .chunks(2)
        .map(|c| (String::from_utf8_lossy(&c[0]).into_owned(), c[1].clone()))
        .collect();
    Ok(Command::HSet(key, fields))
}

pub fn parse_hsetnx(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("HSETNX".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let field = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::HSetnx(key, field, args[2].clone()))
}

pub fn parse_hget(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("HGET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let field = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::HGet(key, field))
}

pub fn parse_hmget(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("HMGET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let fields: Vec<String> = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::HMGet(key, fields))
}

pub fn parse_hmset(args: &[Bytes]) -> Result<Command> {
    if args.len() < 3 || args.len().is_multiple_of(2) {
        return Err(RedisError::WrongArity("HMSET".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let fields: Vec<(String, Bytes)> = args[1..]
        .chunks(2)
        .map(|c| (String::from_utf8_lossy(&c[0]).into_owned(), c[1].clone()))
        .collect();
    Ok(Command::HMSet(key, fields))
}

pub fn parse_hdel(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("HDEL".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let fields: Vec<String> = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::HDel(key, fields))
}

pub fn parse_hexists(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("HEXISTS".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let field = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::HExists(key, field))
}

pub fn parse_hgetall(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("HGETALL".into()));
    }
    Ok(Command::HGetAll(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_hkeys(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("HKEYS".into()));
    }
    Ok(Command::HKeys(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_hvals(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("HVALS".into()));
    }
    Ok(Command::HVals(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_hlen(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("HLEN".into()));
    }
    Ok(Command::HLen(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_hincrby(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("HINCRBY".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let field = String::from_utf8_lossy(&args[1]).into_owned();
    let delta = parse_i64(&args[2], "HINCRBY")?;
    Ok(Command::HIncrBy(key, field, delta))
}

pub fn parse_hincrbyfloat(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("HINCRBYFLOAT".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let field = String::from_utf8_lossy(&args[1]).into_owned();
    let delta = parse_f64(&args[2], "HINCRBYFLOAT")?;
    Ok(Command::HIncrByFloat(key, field, delta))
}

pub fn execute(
    cmd: Command,
    db: &SharedDb,
    db_index: &mut usize,
    script_engine: &Rc<crate::script::ScriptEngine>,
) -> Frame {
    match cmd {
        Command::HSet(key, fields) => {
            let fields: Vec<(String, Vec<u8>)> =
                fields.into_iter().map(|(f, v)| (f, v.to_vec())).collect();
            int_result(
                db.borrow_mut()
                    .hset(*db_index, &key, fields)
                    .map(|n| n as i64),
            )
        }
        Command::HSetnx(key, field, val) => {
            bool_int_result(
                db.borrow_mut()
                    .hsetnx(*db_index, &key, &field, val.to_vec()),
            )
        }
        Command::HGet(key, field) => match db.borrow_mut().hget(*db_index, &key, &field) {
            Ok(Some(v)) => Frame::bulk_bytes(v),
            Ok(None) => Frame::Null,
            Err(e) => Frame::from_error(&e),
        },
        Command::HMGet(key, fields) => match db.borrow_mut().hmget(*db_index, &key, &fields) {
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
        Command::HMSet(key, fields) => {
            // HMSET 与 HSET 行为相同（deprecated 但仍兼容）
            let fields: Vec<(String, Vec<u8>)> =
                fields.into_iter().map(|(f, v)| (f, v.to_vec())).collect();
            ok_result(db.borrow_mut().hset(*db_index, &key, fields).map(|_| ()))
        }
        Command::HDel(key, fields) => int_result(
            db.borrow_mut()
                .hdel(*db_index, &key, &fields)
                .map(|n| n as i64),
        ),
        Command::HExists(key, field) => {
            bool_int_result(db.borrow_mut().hexists(*db_index, &key, &field))
        }
        Command::HGetAll(key) => match db.borrow_mut().hgetall(*db_index, &key) {
            Ok(pairs) => Frame::Array(
                pairs
                    .into_iter()
                    .flat_map(|(f, v)| vec![Frame::bulk_str(f), Frame::bulk_bytes(v)])
                    .collect(),
            ),
            Err(e) => Frame::from_error(&e),
        },
        Command::HKeys(key) => match db.borrow_mut().hkeys(*db_index, &key) {
            Ok(keys) => Frame::Array(keys.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::HVals(key) => match db.borrow_mut().hvals(*db_index, &key) {
            Ok(vals) => Frame::Array(vals.into_iter().map(Frame::bulk_bytes).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::HLen(key) => int_result(db.borrow_mut().hlen(*db_index, &key).map(|n| n as i64)),
        Command::HIncrBy(key, field, delta) => {
            int_result(db.borrow_mut().hincrby(*db_index, &key, &field, delta))
        }
        Command::HIncrByFloat(key, field, delta) => {
            match db.borrow_mut().hincrbyfloat(*db_index, &key, &field, delta) {
                Ok(v) => Frame::bulk_str(format_float(v)),
                Err(e) => Frame::from_error(&e),
            }
        }

        cmd => crate::cmd::set::execute(cmd, db, db_index, script_engine),
    }
}
