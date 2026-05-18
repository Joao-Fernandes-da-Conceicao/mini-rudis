use crate::cmd::{parse_i64, Command};
use crate::error::{RedisError, Result};
use bytes::Bytes;

pub fn parse_ping(args: &[Bytes]) -> Result<Command> {
    match args.len() {
        0 => Ok(Command::Ping(None)),
        1 => Ok(Command::Ping(Some(args[0].clone()))),
        _ => Err(RedisError::WrongArity("PING".into())),
    }
}

pub fn parse_echo(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("ECHO".into()));
    }
    Ok(Command::Echo(args[0].clone()))
}

pub fn parse_select(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("SELECT".into()));
    }
    let n = parse_i64(&args[0], "SELECT")?;
    if !(0..16).contains(&n) {
        return Err(RedisError::DbIndexOutOfRange);
    }
    Ok(Command::Select(n as usize))
}

pub fn parse_del(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("DEL".into()));
    }
    let keys = args
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::Del(keys))
}

pub fn parse_exists(args: &[Bytes]) -> Result<Command> {
    if args.is_empty() {
        return Err(RedisError::WrongArity("EXISTS".into()));
    }
    let keys = args
        .iter()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect();
    Ok(Command::Exists(keys))
}

pub fn parse_type(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("TYPE".into()));
    }
    Ok(Command::Type(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_rename(args: &[Bytes], nx: bool) -> Result<Command> {
    if args.len() != 2 {
        return Err(RedisError::WrongArity(
            if nx { "RENAMENX" } else { "RENAME" }.into(),
        ));
    }
    let src = String::from_utf8_lossy(&args[0]).into_owned();
    let dst = String::from_utf8_lossy(&args[1]).into_owned();
    if nx {
        Ok(Command::Renamenx(src, dst))
    } else {
        Ok(Command::Rename(src, dst))
    }
}

pub fn parse_expire(args: &[Bytes], _at: bool, _p: bool) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("EXPIRE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let secs = parse_i64(&args[1], "EXPIRE")?;
    Ok(Command::Expire(key, secs))
}

pub fn parse_pexpire(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("PEXPIRE".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let ms = parse_i64(&args[1], "PEXPIRE")?;
    Ok(Command::PExpire(key, ms))
}

pub fn parse_pexpireat(args: &[Bytes]) -> Result<Command> {
    if args.len() < 2 {
        return Err(RedisError::WrongArity("PEXPIREAT".into()));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    let ts = parse_i64(&args[1], "PEXPIREAT")?;
    Ok(Command::PExpireAt(key, ts))
}

pub fn parse_ttl(args: &[Bytes], pttl: bool) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity(
            if pttl { "PTTL" } else { "TTL" }.into(),
        ));
    }
    let key = String::from_utf8_lossy(&args[0]).into_owned();
    if pttl {
        Ok(Command::Pttl(key))
    } else {
        Ok(Command::Ttl(key))
    }
}

pub fn parse_persist(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("PERSIST".into()));
    }
    Ok(Command::Persist(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}

pub fn parse_keys(args: &[Bytes]) -> Result<Command> {
    if args.len() != 1 {
        return Err(RedisError::WrongArity("KEYS".into()));
    }
    Ok(Command::Keys(
        String::from_utf8_lossy(&args[0]).into_owned(),
    ))
}
