use bytes::Bytes;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::cmd::{int_result, parse_f64, parse_i64, parse_usize, Command};
use crate::db::{format_float, Database, ScoreBound};
use crate::error::{RedisError, Result};
use crate::proto::Frame;

#[derive(Debug)]
pub struct ZAddArgs {
    pub key: String,
    pub members: Vec<(f64, String)>,
    pub nx: bool,
    pub xx: bool,
    pub gt: bool,
    pub lt: bool,
    pub ch: bool,
}

#[derive(Debug)]
pub struct ZRangeByScoreArgs {
    pub key: String,
    pub min: String,
    pub max: String,
    pub rev: bool,
    pub withscores: bool,
    pub offset: usize,
    pub count: Option<usize>,
}

pub fn parse_zadd(args: &[Bytes]) -> Result<Command> {
    if args.len() < 3 {
        return Err(RedisError::WrongArity("ZADD".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let mut nx = false;
    let mut xx = false;
    let mut gt = false;
    let mut lt = false;
    let mut ch = false;
    let mut i = 1;
    // 解析标志位
    while i < args.len() {
        let opt = String::from_utf8_lossy(&args[i]).to_uppercase();
        match opt.as_str() {
            "NX" => nx = true,
            "XX" => xx = true,
            "GT" => gt = true,
            "LT" => lt = true,
            "CH" => ch = true,
            _ => break,
        }
        i += 1;
    }
    // 剩余必须是 score member 对
    if (args.len() - i) < 2 || !(args.len() - i).is_multiple_of(2) {
        return Err(RedisError::WrongArity("ZADD".into()));
    }
    let mut members = Vec::new();
    while i + 1 < args.len() {
        let score = parse_f64(&args[i], "ZADD")?;
        let member = String::from_utf8_lossy(&args[i + 1]).into_owned();
        members.push((score, member));
        i += 2;
    }
    if (nx && xx) || (gt && lt) {
        return Err(RedisError::SyntaxError);
    }
    Ok(Command::ZAdd(ZAddArgs {
        key,
        members,
        nx,
        xx,
        gt,
        lt,
        ch,
    }))
}

pub fn parse_zrem(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("ZREM".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let members = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::ZRem(key, members))
}

pub fn parse_zscore(args: &[Bytes]) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity("ZSCORE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let member = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::ZScore(key, member))
}

pub fn parse_zmscore(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("ZMSCORE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let members = args[1..]
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::ZMScore(key, members))
}

pub fn parse_zincrby(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("ZINCRBY".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let delta = parse_f64(&args[1], "ZINCRBY")?;
    let member = String::from_utf8_lossy(&args[2]).into_owned();
    Ok(Command::ZIncrBy(key, delta, member))
}

pub fn parse_zrank(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("ZRANK".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let member = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::ZRank(key, member))
}

pub fn parse_zrevrank(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("ZREVRANK".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let member = String::from_utf8_lossy(&args[1]).into_owned();
    Ok(Command::ZRevRank(key, member))
}

pub fn parse_zcard(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("ZCARD".into()));
    }
    Ok(Command::ZCard(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_zcount(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("ZCOUNT".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let min = String::from_utf8_lossy(&args[1]).into_owned();
    let max = String::from_utf8_lossy(&args[2]).into_owned();
    Ok(Command::ZCount(key, min, max))
}

pub fn parse_zrange(args: &[Bytes]) -> Result<Command> {
    if args.len() < 3 {
        return Err(RedisError::WrongArity("ZRANGE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let start = parse_i64(&args[1], "ZRANGE")?;
    let stop = parse_i64(&args[2], "ZRANGE")?;
    let mut rev = false;
    let mut withscores = false;
    for arg in &args[3..] {
        let opt = String::from_utf8_lossy(arg).to_uppercase();
        match opt.as_str() {
            "REV" => rev = true,
            "WITHSCORES" => withscores = true,
            _ => {}
        }
    }
    Ok(Command::ZRange(key, start, stop, rev, withscores))
}

pub fn parse_zrangebyscore(args: &[Bytes], rev: bool) -> Result<Command> {
    if args.len() < 3 {
        let name = if rev {
            "ZREVRANGEBYSCORE"
        } else {
            "ZRANGEBYSCORE"
        };
        return Err(RedisError::WrongArity(name.into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    // 正向: min max, 反向: max min (参数顺序不同)
    let (min, max) = if rev {
        (
            String::from_utf8_lossy(&args[2]).into_owned(),
            String::from_utf8_lossy(&args[1]).into_owned(),
        )
    } else {
        (
            String::from_utf8_lossy(&args[1]).into_owned(),
            String::from_utf8_lossy(&args[2]).into_owned(),
        )
    };
    let mut withscores = false;
    let mut offset = 0usize;
    let mut count: Option<usize> = None;
    let mut i = 3;
    while i < args.len() {
        let opt = String::from_utf8_lossy(&args[i]).to_uppercase();
        match opt.as_str() {
            "WITHSCORES" => withscores = true,
            "LIMIT" => {
                i += 1;
                offset = parse_usize(&args[i], "ZRANGEBYSCORE")?;
                i += 1;
                let c = parse_i64(&args[i], "ZRANGEBYSCORE")?;
                count = if c < 0 { None } else { Some(c as usize) };
            }
            _ => return Err(RedisError::SyntaxError),
        }
        i += 1;
    }
    let args = ZRangeByScoreArgs {
        key,
        min,
        max,
        rev,
        withscores,
        offset,
        count,
    };
    if rev {
        Ok(Command::ZRevRangeByScore(args))
    } else {
        Ok(Command::ZRangeByScore(args))
    }
}

pub fn parse_zrevrange(args: &[Bytes]) -> Result<Command> {
    if args.len() < 3 {
        return Err(RedisError::WrongArity("ZREVRANGE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let start = parse_i64(&args[1], "ZREVRANGE")?;
    let stop = parse_i64(&args[2], "ZREVRANGE")?;
    let withscores = args[3..]
        .iter()
        .any(|b| String::from_utf8_lossy(b).to_uppercase() == "WITHSCORES");
    Ok(Command::ZRevRange(key, start, stop, withscores))
}

pub fn parse_zremrangebyrank(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("ZREMRANGEBYRANK".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let start = parse_i64(&args[1], "ZREMRANGEBYRANK")?;
    let stop = parse_i64(&args[2], "ZREMRANGEBYRANK")?;
    Ok(Command::ZRemRangeByRank(key, start, stop))
}

pub fn parse_zremrangebyscore(args: &[Bytes]) -> Result<Command> {
    if args.len() != 3 {
        return Err(RedisError::WrongArity("ZREMRANGEBYSCORE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let min = String::from_utf8_lossy(&args[1]).into_owned();
    let max = String::from_utf8_lossy(&args[2]).into_owned();
    Ok(Command::ZRemRangeByScore(key, min, max))
}

pub fn parse_zpop(args: &[Bytes], min: bool) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity(
            if min { "ZPOPMIN" } else { "ZPOPMAX" }.into(),
        ));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let count = if args.len() >= 2 {
        parse_usize(&args[1], "ZPOP")?
    } else {
        1
    };
    if min {
        Ok(Command::ZPopMin(key, count))
    } else {
        Ok(Command::ZPopMax(key, count))
    }
}

pub fn execute(
    cmd: Command,
    db: &Arc<Mutex<Database>>,
    db_index: &mut usize,
    script_engine: &Arc<crate::script::ScriptEngine>,
) -> Frame {
    match cmd {
        Command::ZAdd(args) => int_result(db.lock().zadd(
            *db_index,
            &args.key,
            args.members,
            args.nx,
            args.xx,
            args.gt,
            args.lt,
            args.ch,
        )),
        Command::ZRem(key, members) => {
            int_result(db.lock().zrem(*db_index, &key, &members).map(|n| n as i64))
        }
        Command::ZScore(key, member) => match db.lock().zscore(*db_index, &key, &member) {
            Ok(Some(s)) => Frame::bulk_str(format_float(s)),
            Ok(None) => Frame::Null,
            Err(e) => Frame::from_error(&e),
        },
        Command::ZMScore(key, members) => match db.lock().zmscore(*db_index, &key, &members) {
            Ok(scores) => Frame::Array(
                scores
                    .into_iter()
                    .map(|s| match s {
                        Some(f) => Frame::bulk_str(format_float(f)),
                        None => Frame::Null,
                    })
                    .collect(),
            ),
            Err(e) => Frame::from_error(&e),
        },
        Command::ZIncrBy(key, delta, member) => {
            match db.lock().zincrby(*db_index, &key, delta, &member) {
                Ok(score) => Frame::bulk_str(format_float(score)),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ZRank(key, member) => match db.lock().zrank(*db_index, &key, &member) {
            Ok(Some(r)) => Frame::Integer(r as i64),
            Ok(None) => Frame::Null,
            Err(e) => Frame::from_error(&e),
        },
        Command::ZRevRank(key, member) => match db.lock().zrevrank(*db_index, &key, &member) {
            Ok(Some(r)) => Frame::Integer(r as i64),
            Ok(None) => Frame::Null,
            Err(e) => Frame::from_error(&e),
        },
        Command::ZCard(key) => int_result(db.lock().zcard(*db_index, &key).map(|n| n as i64)),
        Command::ZCount(key, min_s, max_s) => {
            let min = match ScoreBound::parse_min(&min_s) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            let max = match ScoreBound::parse_max(&max_s) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            int_result(
                db.lock()
                    .zcount(*db_index, &key, min, max)
                    .map(|n| n as i64),
            )
        }
        Command::ZRange(key, start, stop, rev, withscores) => {
            match db
                .lock()
                .zrange(*db_index, &key, start, stop, rev, withscores)
            {
                Ok(items) => build_member_score_array(items),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ZRangeByScore(args) => {
            let min = match ScoreBound::parse_min(&args.min) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            let max = match ScoreBound::parse_max(&args.max) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            match db.lock().zrangebyscore(
                *db_index,
                &args.key,
                min,
                max,
                args.withscores,
                args.offset,
                args.count,
            ) {
                Ok(items) => build_member_score_array(items),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ZRevRangeByScore(args) => {
            let max = match ScoreBound::parse_max(&args.max) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            let min = match ScoreBound::parse_min(&args.min) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            match db.lock().zrevrangebyscore(
                *db_index,
                &args.key,
                max,
                min,
                args.withscores,
                args.offset,
                args.count,
            ) {
                Ok(items) => build_member_score_array(items),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ZRevRange(key, start, stop, withscores) => {
            match db
                .lock()
                .zrange(*db_index, &key, start, stop, true, withscores)
            {
                Ok(items) => build_member_score_array(items),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ZRemRangeByRank(key, start, stop) => int_result(
            db.lock()
                .zremrangebyrank(*db_index, &key, start, stop)
                .map(|n| n as i64),
        ),
        Command::ZRemRangeByScore(key, min_s, max_s) => {
            let min = match ScoreBound::parse_min(&min_s) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            let max = match ScoreBound::parse_max(&max_s) {
                Ok(b) => b,
                Err(e) => return Frame::from_error(&e),
            };
            int_result(
                db.lock()
                    .zremrangebyscore(*db_index, &key, min, max)
                    .map(|n| n as i64),
            )
        }
        Command::ZPopMin(key, count) => match db.lock().zpopmin(*db_index, &key, count) {
            Ok(items) => Frame::Array(
                items
                    .into_iter()
                    .flat_map(|(m, s)| vec![Frame::bulk_str(m), Frame::bulk_str(format_float(s))])
                    .collect(),
            ),
            Err(e) => Frame::from_error(&e),
        },
        Command::ZPopMax(key, count) => match db.lock().zpopmax(*db_index, &key, count) {
            Ok(items) => Frame::Array(
                items
                    .into_iter()
                    .flat_map(|(m, s)| vec![Frame::bulk_str(m), Frame::bulk_str(format_float(s))])
                    .collect(),
            ),
            Err(e) => Frame::from_error(&e),
        },

        // ── 脚本命令最终落地 ────────────────────────────────────────
        Command::Eval(script, _numkeys, keys, argv) => {
            let keys: Vec<String> = keys
                .iter()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect();
            let argv: Vec<String> = argv
                .iter()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect();
            match script_engine.eval(
                &String::from_utf8_lossy(&script),
                &keys,
                &argv,
                db,
                db_index,
            ) {
                Ok(f) => f,
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::EvalSha(sha, _numkeys, keys, argv) => {
            let sha_str = String::from_utf8_lossy(&sha).into_owned();
            let keys: Vec<String> = keys
                .iter()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect();
            let argv: Vec<String> = argv
                .iter()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect();
            match script_engine.evalsha(&sha_str, &keys, &argv, db, db_index) {
                Ok(f) => f,
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ScriptLoad(script) => {
            match script_engine.load_script(&String::from_utf8_lossy(&script)) {
                Ok(sha) => Frame::bulk_str(sha),
                Err(e) => Frame::from_error(&e),
            }
        }
        Command::ScriptFlush => {
            script_engine.flush();
            Frame::ok()
        }

        _ => Frame::Error("ERR unknown command in final dispatch".to_string()),
    }
}

/// 将 `(member, Option<score>)` 列表转换为 RESP 数组
fn build_member_score_array(items: Vec<(String, Option<f64>)>) -> Frame {
    let mut frames = Vec::with_capacity(items.len() * 2);
    for (member, score) in items {
        frames.push(Frame::bulk_str(member));
        if let Some(s) = score {
            frames.push(Frame::bulk_str(format_float(s)));
        }
    }
    Frame::Array(frames)
}
