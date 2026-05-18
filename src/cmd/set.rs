use bytes::Bytes;
use std::rc::Rc;

use crate::cmd::{bool_int_result, int_result, parse_i64, Command};
use crate::error::{RedisError, Result};
use crate::proto::Frame;
use crate::SharedDb;

pub fn parse_sadd(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("SADD".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let members: Vec<String> = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::SAdd(key, members))
}

pub fn parse_srem(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("SREM".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let members: Vec<String> = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::SRem(key, members))
}

pub fn parse_smembers(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("SMEMBERS".into()));
    }
    Ok(Command::SMembers(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_sismember(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("SISMEMBER".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let member = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::SIsMember(key, member))
}

pub fn parse_smismember(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("SMISMEMBER".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let members: Vec<String> = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::SMIsMember(key, members))
}

pub fn parse_scard(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("SCARD".into()));
    }
    Ok(Command::SCard(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_sunion(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("SUNION".into()));
    }
    Ok(Command::SUnion(
        args.iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect(),
    ))
}

pub fn parse_sinter(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("SINTER".into()));
    }
    Ok(Command::SInter(
        args.iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect(),
    ))
}

pub fn parse_sdiff(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("SDIFF".into()));
    }
    Ok(Command::SDiff(
        args.iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect(),
    ))
}

pub fn parse_store(args: &[Bytes], cmd: &str) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity(cmd.into()));
    }
    let dst = String::from_utf8_lossy(&args[0]).into_owned();
    let keys: Vec<String> = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    match cmd {
        "SUNIONSTORE" => Ok(Command::SUnionStore(dst, keys)),
        "SINTERSTORE" => Ok(Command::SInterStore(dst, keys)),
        "SDIFFSTORE" => Ok(Command::SDiffStore(dst, keys)),
        _ => unreachable!(),
    }
}

pub fn parse_spop(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("SPOP".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let count = if args.len() >= 2 {
        let n = parse_i64(&args[1], "SPOP")?;
        if n < 0 {
            return Err(RedisError::Generic(
                "ERR value is out of range, must be a positive integer or zero".into(),
            ));
        }
        n as usize
    } else {
        1
    };
    Ok(Command::SPop(key, count))
}

pub fn parse_srandmember(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("SRANDMEMBER".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let count = if args.len() >= 2 {
        parse_i64(&args[1], "SRANDMEMBER")?
    } else {
        1
    };
    Ok(Command::SRandMember(key, count))
}

pub fn parse_smove(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("SMOVE".into()));
    }
    let src = String::from_utf8_lossy(&args[0]).into_owned();
    let dst = String::from_utf8_lossy(&args[1]).into_owned();
    let member = String::from_utf8_lossy(&args[2]).into_owned();
    Ok(Command::SMove(src, dst, member))
}

pub fn execute(
    cmd: Command,
    db: &SharedDb,
    db_index: &mut usize,
    script_engine: &Rc<crate::script::ScriptEngine>,
) -> Frame {
    match cmd {
        Command::SAdd(key, members) => {
            int_result(db.borrow_mut().sadd(*db_index, &key, members).map(|n| n as i64))
        }
        Command::SRem(key, members) => {
            int_result(db.borrow_mut().srem(*db_index, &key, &members).map(|n| n as i64))
        }
        Command::SMembers(key) => match db.borrow_mut().smembers(*db_index, &key) {
            Ok(members) => Frame::Array(members.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::SIsMember(key, member) => {
            bool_int_result(db.borrow_mut().sismember(*db_index, &key, &member))
        }
        Command::SMIsMember(key, members) => {
            match db.borrow_mut().smismember(*db_index, &key, &members) {
                Ok(results) => Frame::Array(
                    results
                        .into_iter()
                        .map(|b| Frame::Integer(if b { 1 } else { 0 }))
                        .collect(),
                ),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::SCard(key) => int_result(db.borrow_mut().scard(*db_index, &key).map(|n| n as i64)),
        Command::SUnion(keys) => match db.borrow_mut().sunion(*db_index, &keys) {
            Ok(members) => Frame::Array(members.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::SInter(keys) => match db.borrow_mut().sinter(*db_index, &keys) {
            Ok(members) => Frame::Array(members.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::SDiff(keys) => match db.borrow_mut().sdiff(*db_index, &keys) {
            Ok(members) => Frame::Array(members.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::SUnionStore(dst, keys) => int_result(
            db.borrow_mut()
                .sunionstore(*db_index, &dst, &keys)
                .map(|n| n as i64),
        ),
        Command::SInterStore(dst, keys) => int_result(
            db.borrow_mut()
                .sinterstore(*db_index, &dst, &keys)
                .map(|n| n as i64),
        ),
        Command::SDiffStore(dst, keys) => int_result(
            db.borrow_mut()
                .sdiffstore(*db_index, &dst, &keys)
                .map(|n| n as i64),
        ),
        Command::SPop(key, count) => match db.borrow_mut().spop(*db_index, &key, count) {
            Ok(members) if count == 1 => match members.into_iter().next() {
                Some(m) => Frame::bulk_str(m),
                None => Frame::Null,
            },
            Ok(members) => Frame::Array(members.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::SRandMember(key, count) => match db.borrow_mut().srandmember(*db_index, &key, count) {
            Ok(members) if count == 1 => match members.into_iter().next() {
                Some(m) => Frame::bulk_str(m),
                None => Frame::Null,
            },
            Ok(members) => Frame::Array(members.into_iter().map(Frame::bulk_str).collect()),
            Err(e) => Frame::from_error(&e),
        },
        Command::SMove(src, dst, member) => {
            bool_int_result(db.borrow_mut().smove(*db_index, &src, &dst, &member))
        }

        cmd => crate::cmd::zset::execute(cmd, db, db_index, script_engine),
    }
}
