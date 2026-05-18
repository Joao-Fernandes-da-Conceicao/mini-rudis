/// 持久化层接口定义（RDB / AOF）
///
/// 本版本仅预留接口，不实现具体逻辑。
pub mod aof;
pub mod rdb;

use crate::error::Result;

/// 持久化后端抽象 trait
pub trait Persistence: Send + Sync {
    /// 将当前数据库状态快照保存到磁盘
    fn save(&self) -> Result<()>;
    /// 从磁盘加载数据并恢复数据库状态
    fn load(&self) -> Result<()>;
    /// 返回上次成功保存的 Unix 时间戳（秒）
    fn last_save(&self) -> u64;
}
