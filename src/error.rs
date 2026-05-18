use thiserror::Error;

/// 所有错误类型的统一枚举
#[derive(Debug, Error)]
pub enum RedisError {
    #[error("ERR wrong number of arguments for '{0}' command")]
    WrongArity(String),

    #[error("ERR value is not an integer or out of range")]
    NotInteger,

    #[error("ERR value is not a valid float")]
    NotFloat,

    #[error("WRONGTYPE Operation against a key holding the wrong kind of value")]
    WrongType,

    #[error("ERR {0}")]
    Generic(String),

    #[error("ERR no such key")]
    NoSuchKey,

    #[error("ERR index out of range")]
    IndexOutOfRange,

    #[error("ERR syntax error")]
    SyntaxError,

    #[error("ERR Protocol error: {0}")]
    Protocol(String),

    #[error("ERR Lua script error: {0}")]
    Script(String),

    #[error("ERR DB index is out of range")]
    DbIndexOutOfRange,

    #[error("ERR NaN is not allowed as a score")]
    ScoreNaN,

    #[error("ERR no more elements")]
    Exhausted,

    #[error("ERR source and destination objects are the same")]
    SameObject,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RedisError>;
