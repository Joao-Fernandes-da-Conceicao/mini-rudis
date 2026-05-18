/// Lua 脚本执行引擎
///
/// 实现 EVAL / EVALSHA / SCRIPT LOAD / SCRIPT FLUSH 命令。
/// 每次 EVAL 调用都在一个干净的 Lua 环境中执行，KEYS 和 ARGV 全局变量已注入。
/// `redis.call()` 和 `redis.pcall()` 会转发到数据库层执行。
use std::collections::HashMap;
use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::Mutex;
use sha1::{Digest, Sha1};

use crate::db::Database;
use crate::error::{RedisError, Result};
use crate::proto::Frame;

/// 脚本引擎：管理脚本缓存并提供执行能力
pub struct ScriptEngine {
    /// SHA1 → 脚本源码 的缓存
    cache: Mutex<HashMap<String, String>>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        ScriptEngine {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 计算脚本的 SHA1 摘要（与 Redis 兼容的小写十六进制字符串）
    pub fn sha1_hex(script: &str) -> String {
        let mut hasher = Sha1::new();
        hasher.update(script.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 加载脚本到缓存并返回 SHA1
    pub fn load_script(&self, script: &str) -> Result<String> {
        let sha = Self::sha1_hex(script);
        self.cache.lock().insert(sha.clone(), script.to_owned());
        Ok(sha)
    }

    /// 清空脚本缓存
    pub fn flush(&self) {
        self.cache.lock().clear();
    }

    /// 通过 SHA1 执行脚本
    pub fn evalsha(
        &self,
        sha: &str,
        keys: &[String],
        argv: &[String],
        db: &Arc<Mutex<Database>>,
        db_index: &mut usize,
    ) -> Result<Frame> {
        let script = self.cache.lock().get(sha).cloned().ok_or_else(|| {
            RedisError::Generic("NOSCRIPT No matching script. Please use EVAL.".into())
        })?;
        self.eval(&script, keys, argv, db, db_index)
    }

    /// 执行 Lua 脚本
    pub fn eval(
        &self,
        script: &str,
        keys: &[String],
        argv: &[String],
        db: &Arc<Mutex<Database>>,
        db_index: &mut usize,
    ) -> Result<Frame> {
        // 每次 eval 创建新的 Lua 虚拟机以隔离状态
        let lua = Lua::new();
        self.setup_lua_env(&lua, keys, argv, db, db_index)?;

        let result: LuaValue = lua
            .load(script)
            .eval()
            .map_err(|e| RedisError::Script(e.to_string()))?;

        lua_value_to_frame(result)
    }

    /// 初始化 Lua 环境：注入 KEYS、ARGV 和 redis 全局对象
    fn setup_lua_env(
        &self,
        lua: &Lua,
        keys: &[String],
        argv: &[String],
        db: &Arc<Mutex<Database>>,
        db_index: &mut usize,
    ) -> Result<()> {
        let globals = lua.globals();

        // 注入 KEYS 表（1-indexed）
        let keys_table = lua.create_table().map_err(lua_err)?;
        for (i, k) in keys.iter().enumerate() {
            keys_table.set(i + 1, k.as_str()).map_err(lua_err)?;
        }
        globals.set("KEYS", keys_table).map_err(lua_err)?;

        // 注入 ARGV 表（1-indexed）
        let argv_table = lua.create_table().map_err(lua_err)?;
        for (i, a) in argv.iter().enumerate() {
            argv_table.set(i + 1, a.as_str()).map_err(lua_err)?;
        }
        globals.set("ARGV", argv_table).map_err(lua_err)?;

        // 构造 redis.call / redis.pcall
        let db_clone = Arc::clone(db);
        let db_idx = *db_index;

        let call_fn = lua
            .create_function(move |lua, args: LuaMultiValue| {
                exec_redis_call(lua, args, &db_clone, db_idx, false)
            })
            .map_err(lua_err)?;

        let db_clone2 = Arc::clone(db);
        let pcall_fn = lua
            .create_function(move |lua, args: LuaMultiValue| {
                exec_redis_call(lua, args, &db_clone2, db_idx, true)
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

/// 在 Lua 中执行一条 Redis 命令（redis.call 的实现）
fn exec_redis_call(
    lua: &Lua,
    args: LuaMultiValue,
    db: &Arc<Mutex<Database>>,
    db_index: usize,
    pcall: bool,
) -> LuaResult<LuaMultiValue> {
    if args.is_empty() {
        return Err(LuaError::RuntimeError("redis.call: missing command".into()));
    }

    // 将 Lua 参数转换为字节串向量
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

    let script_engine = Arc::new(ScriptEngine::new());
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

/// 将 RESP Frame 转换为 Lua 值
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

/// 将 Lua 值转换为 RESP Frame（脚本返回值）
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
            // 检查是否是错误表 {err = "..."}
            if let Ok(LuaValue::String(err)) = t.get("err") {
                return Ok(Frame::Error(err.to_string_lossy().to_owned()));
            }
            // 检查是否是状态表 {ok = "..."}
            if let Ok(LuaValue::String(ok)) = t.get("ok") {
                return Ok(Frame::Simple(ok.to_string_lossy().to_owned()));
            }
            // 普通数组（1-indexed）
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
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn make_db() -> Arc<Mutex<Database>> {
        Arc::new(Mutex::new(Database::new(16)))
    }

    #[test]
    fn test_sha1_hex() {
        let sha = ScriptEngine::sha1_hex("return 1");
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_eval_return_integer() {
        let engine = ScriptEngine::new();
        let db = make_db();
        let mut idx = 0;
        let frame = engine.eval("return 42", &[], &[], &db, &mut idx).unwrap();
        assert_eq!(frame, Frame::Integer(42));
    }

    #[test]
    fn test_eval_return_string() {
        let engine = ScriptEngine::new();
        let db = make_db();
        let mut idx = 0;
        let frame = engine
            .eval("return 'hello'", &[], &[], &db, &mut idx)
            .unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("hello")));
    }

    #[test]
    fn test_eval_keys_argv() {
        let engine = ScriptEngine::new();
        let db = make_db();
        let mut idx = 0;
        let frame = engine
            .eval("return KEYS[1]", &["mykey".into()], &[], &db, &mut idx)
            .unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("mykey")));
    }

    #[test]
    fn test_script_load_and_evalsha() {
        let engine = ScriptEngine::new();
        let db = make_db();
        let mut idx = 0;
        let sha = engine.load_script("return 'loaded'").unwrap();
        assert_eq!(sha.len(), 40);
        let frame = engine.evalsha(&sha, &[], &[], &db, &mut idx).unwrap();
        assert_eq!(frame, Frame::Bulk(bytes::Bytes::from("loaded")));
    }

    #[test]
    fn test_eval_redis_set_get() {
        let engine = ScriptEngine::new();
        let db = make_db();
        let mut idx = 0;
        let frame = engine
            .eval(
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
