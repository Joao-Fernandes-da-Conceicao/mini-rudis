/// Lua 脚本执行引擎（单线程 `LocalSet` 语义下使用）。
///
/// `redis.call` / `redis.pcall` 与命令层共享同一 `Rc<RefCell<Database>>`，无需 Mutex。
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mlua::prelude::*;
use sha1::{Digest, Sha1};

use crate::error::{RedisError, Result};
use crate::proto::Frame;
use crate::SharedDb;

/// 脚本引擎：管理脚本缓存并提供执行能力。
pub struct ScriptEngine {
    /// SHA1 → 源码
    cache: RefCell<HashMap<String, String>>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        ScriptEngine {
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// 计算脚本 SHA1（小写十六进制，与 Redis 一致）
    pub fn sha1_hex(script: &str) -> String {
        let mut hasher = Sha1::new();
        hasher.update(script.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 载入缓存并返回 SHA1
    pub fn load_script(&self, script: &str) -> Result<String> {
        let sha = Self::sha1_hex(script);
        self.cache.borrow_mut().insert(sha.clone(), script.to_owned());
        Ok(sha)
    }

    pub fn flush(&self) {
        self.cache.borrow_mut().clear();
    }

    /// 按 SHA1 执行已缓存脚本
    pub fn evalsha(
        engine: &Rc<Self>,
        sha: &str,
        keys: &[String],
        argv: &[String],
        db: &SharedDb,
        db_index: &mut usize,
    ) -> Result<Frame> {
        let script = engine
            .cache
            .borrow()
            .get(sha)
            .cloned()
            .ok_or_else(|| {
                RedisError::Generic("NOSCRIPT No matching script. Please use EVAL.".into())
            })?;
        Self::eval(engine, &script, keys, argv, db, db_index)
    }

    /// 执行 Lua 源码
    pub fn eval(
        engine: &Rc<Self>,
        script: &str,
        keys: &[String],
        argv: &[String],
        db: &SharedDb,
        db_index: &mut usize,
    ) -> Result<Frame> {
        let lua = Lua::new();
        Self::inject_redis_env(engine, &lua, keys, argv, db, *db_index)?;

        let result: LuaValue = lua
            .load(script)
            .eval()
            .map_err(|e| RedisError::Script(e.to_string()))?;

        lua_value_to_frame(result)
    }

    fn inject_redis_env(
        engine: &Rc<Self>,
        lua: &Lua,
        keys: &[String],
        argv: &[String],
        db: &SharedDb,
        db_index: usize,
    ) -> Result<()> {
        let globals = lua.globals();

        let keys_table = lua.create_table().map_err(lua_err)?;
        for (i, k) in keys.iter().enumerate() {
            keys_table.set(i + 1, k.as_str()).map_err(lua_err)?;
        }
        globals.set("KEYS", keys_table).map_err(lua_err)?;

        let argv_table = lua.create_table().map_err(lua_err)?;
        for (i, a) in argv.iter().enumerate() {
            argv_table.set(i + 1, a.as_str()).map_err(lua_err)?;
        }
        globals.set("ARGV", argv_table).map_err(lua_err)?;

        let db_call = Rc::clone(db);
        let se_call = Rc::clone(engine);
        let call_fn = lua
            .create_function(move |lua, args: LuaMultiValue| {
                exec_redis_call(lua, args, &db_call, db_index, false, Rc::clone(&se_call))
            })
            .map_err(lua_err)?;

        let db_p = Rc::clone(db);
        let se_p = Rc::clone(engine);
        let pcall_fn = lua
            .create_function(move |lua, args: LuaMultiValue| {
                exec_redis_call(lua, args, &db_p, db_index, true, Rc::clone(&se_p))
            })
            .map_err(lua_err)?;

        let redis_table = lua.create_table().map_err(lua_err)?;
        redis_table.set("call", call_fn).map_err(lua_err)?;
        redis_table.set("pcall", pcall_fn).map_err(lua_err)?;
        redis_table
            .set(
                "error_reply",
                lua.create_function(|lua, msg: String| {
                    let t = lua.create_table()?;
                    t.set("err", msg)?;
                    Ok(t)
                })
                .map_err(lua_err)?,
            )
            .map_err(lua_err)?;
        globals.set("redis", redis_table).map_err(lua_err)?;

        Ok(())
    }
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn exec_redis_call(
    lua: &Lua,
    args: LuaMultiValue,
    db: &SharedDb,
    db_index: usize,
    pcall: bool,
    script_engine: Rc<ScriptEngine>,
) -> LuaResult<LuaMultiValue> {
    if args.is_empty() {
        return Err(LuaError::RuntimeError("redis.call: missing command".into()));
    }

    let cmd_args: Vec<bytes::Bytes> = args
        .iter()
        .map(|v| match v {
            LuaValue::String(s) => Ok(bytes::Bytes::from(s.as_bytes().to_vec())),
            LuaValue::Integer(n) => Ok(bytes::Bytes::from(n.to_string())),
            LuaValue::Number(f) => Ok(bytes::Bytes::from(crate::db::format_float(*f).into_bytes())),
            other => Err(LuaError::RuntimeError(format!(
                "redis.call: unsupported argument type: {:?}",
                other.type_name()
            ))),
        })
        .collect::<LuaResult<_>>()?;

    let cmd = crate::cmd::parse(&cmd_args).map_err(|e| LuaError::RuntimeError(e.to_string()))?;

    let mut idx = db_index;
    let frame = crate::cmd::execute(cmd, db, &mut idx, &script_engine);

    match &frame {
        Frame::Error(msg) if !pcall => {
            return Err(LuaError::RuntimeError(msg.clone()));
        }
        _ => {}
    }

    let lua_val = frame_to_lua_value(lua, frame)?;
    Ok(LuaMultiValue::from_vec(vec![lua_val]))
}

fn frame_to_lua_value(lua: &Lua, frame: Frame) -> LuaResult<LuaValue> {
    match frame {
        Frame::Simple(s) => Ok(LuaValue::String(lua.create_string(&s)?)),
        Frame::Error(e) => {
            let t = lua.create_table()?;
            t.set("err", e)?;
            Ok(LuaValue::Table(t))
        }
        Frame::Integer(n) => Ok(LuaValue::Integer(n)),
        Frame::Bulk(b) => Ok(LuaValue::String(lua.create_string(&b)?)),
        Frame::Null => Ok(LuaValue::Boolean(false)),
        Frame::Array(items) => {
            let t = lua.create_table()?;
            for (i, item) in items.into_iter().enumerate() {
                t.set(i + 1, frame_to_lua_value(lua, item)?)?;
            }
            Ok(LuaValue::Table(t))
        }
    }
}

fn lua_value_to_frame(val: LuaValue) -> Result<Frame> {
    match val {
        LuaValue::Nil => Ok(Frame::Null),
        LuaValue::Boolean(b) => {
            if b {
                Ok(Frame::Integer(1))
            } else {
                Ok(Frame::Null)
            }
        }
        LuaValue::Integer(n) => Ok(Frame::Integer(n)),
        LuaValue::Number(f) => Ok(Frame::Integer(f as i64)),
        LuaValue::String(s) => Ok(Frame::bulk_bytes(s.as_bytes().to_vec())),
        LuaValue::Table(t) => {
            if let Ok(LuaValue::String(err)) = t.get("err") {
                return Ok(Frame::Error(err.to_string_lossy().to_owned()));
            }
            if let Ok(LuaValue::String(ok)) = t.get("ok") {
                return Ok(Frame::Simple(ok.to_string_lossy().to_owned()));
            }
            let mut frames = Vec::new();
            let mut i = 1i64;
            loop {
                match t.get::<LuaValue>(i) {
                    Ok(LuaValue::Nil) => break,
                    Ok(v) => frames.push(lua_value_to_frame(v)?),
                    Err(_) => break,
                }
                i += 1;
            }
            Ok(Frame::Array(frames))
        }
        other => Ok(Frame::bulk_str(format!("{:?}", other))),
    }
}

fn lua_err(e: LuaError) -> RedisError {
    RedisError::Script(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn make_db() -> SharedDb {
        Rc::new(RefCell::new(Database::new(16)))
    }

    #[test]
    fn test_sha1_hex() {
        let sha = ScriptEngine::sha1_hex("return 1");
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_eval_return_integer() {
        let engine = Rc::new(ScriptEngine::new());
        let db = make_db();
        let mut idx = 0;
        let frame = ScriptEngine::eval(&engine, "return 42", &[], &[], &db, &mut idx).unwrap();
        assert_eq!(frame, Frame::Integer(42));
    }

    #[test]
    fn test_eval_return_string() {
        let engine = Rc::new(ScriptEngine::new());
        let db = make_db();
        let mut idx = 0;
        let frame = ScriptEngine::eval(&engine, "return 'hello'", &[], &[], &db, &mut idx).unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("hello")));
    }

    #[test]
    fn test_eval_keys_argv() {
        let engine = Rc::new(ScriptEngine::new());
        let db = make_db();
        let mut idx = 0;
        let frame = ScriptEngine::eval(
            &engine,
            "return KEYS[1]",
            &["mykey".into()],
            &[],
            &db,
            &mut idx,
        )
        .unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("mykey")));
    }

    #[test]
    fn test_script_load_and_evalsha() {
        let engine = Rc::new(ScriptEngine::new());
        let db = make_db();
        let mut idx = 0;
        let sha = engine.load_script("return 'loaded'").unwrap();
        assert_eq!(sha.len(), 40);
        let frame =
            ScriptEngine::evalsha(&engine, &sha, &[], &[], &db, &mut idx).unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("loaded")));
    }

    #[test]
    fn test_eval_redis_set_get() {
        let engine = Rc::new(ScriptEngine::new());
        let db = make_db();
        let mut idx = 0;
        let frame = ScriptEngine::eval(
            &engine,
            "redis.call('SET', KEYS[1], ARGV[1]); return redis.call('GET', KEYS[1])",
            &["k".into()],
            &["v".into()],
            &db,
            &mut idx,
        )
        .unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("v")));
    }
}
