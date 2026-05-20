/// 数据库核心层
///
/// 支持 5 种 Redis 数据类型 + TTL 过期机制 + 16 个逻辑数据库
mod skiplist;

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use skiplist::SkipList;

use crate::error::{RedisError, Result};

/// 存储在数据库中的对象类型
#[derive(Debug, Clone)]
pub enum RudisObject {
    Str(Vec<u8>),
    List(VecDeque<Vec<u8>>),
    Hash(HashMap<String, Vec<u8>>),
    Set(HashSet<String>),
    ZSet(ZSetInner),
}

impl RudisObject {
    pub fn type_name(&self) -> &'static str {
        match self {
            RudisObject::Str(_) => "string",
            RudisObject::List(_) => "list",
            RudisObject::Hash(_) => "hash",
            RudisObject::Set(_) => "set",
            RudisObject::ZSet(_) => "zset",
        }
    }
}

/// 有序集合内部数据结构（对齐 Redis：dict + skiplist）
///
/// - `scores`：member → score，O(1) 单点查询（ZSCORE / ZRANK 前置）
/// - `sl`：按 (score, member) 有序的跳表，O(log n) 插入/删除，均摊 O(log n) 范围定位
#[derive(Debug, Clone, Default)]
pub struct ZSetInner {
    pub scores: HashMap<String, f64>,
    pub sl: SkipList,
}

impl ZSetInner {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加或更新成员，返回是否为新增（true）还是更新（false）
    pub fn add(&mut self, member: String, score: f64) -> bool {
        if let Some(&old_score) = self.scores.get(&member) {
            self.sl.remove(old_score, &member);
        }
        let is_new = self.scores.insert(member.clone(), score).is_none();
        self.sl.insert(score, member);
        is_new
    }

    pub fn remove(&mut self, member: &str) -> bool {
        if let Some(score) = self.scores.remove(member) {
            self.sl.remove(score, member);
            true
        } else {
            false
        }
    }

    pub fn score(&self, member: &str) -> Option<f64> {
        self.scores.get(member).copied()
    }

    pub fn len(&self) -> usize {
        self.scores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    /// 返回从小到大第 rank 名（0-indexed），不存在返回 None
    pub fn rank(&self, member: &str) -> Option<usize> {
        let score = *self.scores.get(member)?;
        self.sl.rank_of(score, member)
    }

    /// 返回从大到小第 rank 名（0-indexed）
    pub fn revrank(&self, member: &str) -> Option<usize> {
        let rank = self.rank(member)?;
        Some(self.len() - 1 - rank)
    }

    /// 按索引范围获取元素（从小到大，0-indexed，支持负数）
    pub fn range_by_index(
        &self,
        start: i64,
        stop: i64,
        withscores: bool,
    ) -> Vec<(String, Option<f64>)> {
        let len = self.len() as i64;
        let start = normalize_index(start, len).min(len as usize);
        let stop = normalize_index(stop, len).min(len.saturating_sub(1) as usize);
        if start > stop {
            return vec![];
        }
        self.sl
            .iter()
            .skip(start)
            .take(stop - start + 1)
            .map(|(member, score)| (member, if withscores { Some(score) } else { None }))
            .collect()
    }

    /// 按分数范围获取（含 -inf/+inf 支持）
    pub fn range_by_score(
        &self,
        min: ScoreBound,
        max: ScoreBound,
        withscores: bool,
        offset: usize,
        count: Option<usize>,
    ) -> Vec<(String, Option<f64>)> {
        self.sl
            .iter()
            .filter(|(_, score)| min.contains(*score) && max.contains_upper(*score))
            .skip(offset)
            .take(count.unwrap_or(usize::MAX))
            .map(|(member, score)| (member, if withscores { Some(score) } else { None }))
            .collect()
    }

    /// 按分数范围倒序获取
    pub fn revrange_by_score(
        &self,
        max: ScoreBound,
        min: ScoreBound,
        withscores: bool,
        offset: usize,
        count: Option<usize>,
    ) -> Vec<(String, Option<f64>)> {
        self.sl
            .iter_rev()
            .filter(|(_, score)| min.contains(*score) && max.contains_upper(*score))
            .skip(offset)
            .take(count.unwrap_or(usize::MAX))
            .map(|(member, score)| (member, if withscores { Some(score) } else { None }))
            .collect()
    }

    /// 统计分数在 [min, max] 范围内的元素数
    pub fn count_by_score(&self, min: ScoreBound, max: ScoreBound) -> usize {
        self.sl
            .iter()
            .filter(|(_, score)| min.contains(*score) && max.contains_upper(*score))
            .count()
    }

    /// 移除分数在范围内的元素，返回移除数量
    pub fn remove_by_score(&mut self, min: ScoreBound, max: ScoreBound) -> usize {
        let to_remove: Vec<_> = self
            .sl
            .iter()
            .filter(|(_, score)| min.contains(*score) && max.contains_upper(*score))
            .map(|(member, _)| member)
            .collect();
        let count = to_remove.len();
        for member in to_remove {
            self.remove(&member);
        }
        count
    }

    /// 移除排名在 [start, stop] 范围内的元素（0-indexed）
    pub fn remove_by_rank(&mut self, start: i64, stop: i64) -> usize {
        let len = self.len() as i64;
        let start = normalize_index(start, len).min(len as usize);
        let stop = normalize_index(stop, len).min(len.saturating_sub(1) as usize);
        if start > stop {
            return 0;
        }
        let to_remove: Vec<_> = self
            .sl
            .iter()
            .skip(start)
            .take(stop - start + 1)
            .map(|(member, _)| member)
            .collect();
        let count = to_remove.len();
        for member in to_remove {
            self.remove(&member);
        }
        count
    }

    /// 弹出分数最小的 count 个元素
    pub fn pop_min(&mut self, count: usize) -> Vec<(String, f64)> {
        let keys: Vec<_> = self
            .sl
            .iter()
            .take(count)
            .map(|(member, score)| (member, score))
            .collect();
        for (member, _) in &keys {
            self.remove(member);
        }
        keys
    }

    /// 弹出分数最大的 count 个元素
    pub fn pop_max(&mut self, count: usize) -> Vec<(String, f64)> {
        let keys: Vec<_> = self
            .sl
            .iter_rev()
            .take(count)
            .map(|(member, score)| (member, score))
            .collect();
        for (member, _) in &keys {
            self.remove(member);
        }
        keys
    }
}

/// 分数范围边界（支持 inclusive/exclusive/-inf/+inf）
#[derive(Debug, Clone, Copy)]
pub struct ScoreBound {
    pub value: f64,
    pub exclusive: bool,
    pub is_neg_inf: bool,
    pub is_pos_inf: bool,
}

impl ScoreBound {
    pub fn neg_inf() -> Self {
        ScoreBound {
            value: f64::NEG_INFINITY,
            exclusive: false,
            is_neg_inf: true,
            is_pos_inf: false,
        }
    }

    pub fn pos_inf() -> Self {
        ScoreBound {
            value: f64::INFINITY,
            exclusive: false,
            is_neg_inf: false,
            is_pos_inf: true,
        }
    }

    pub fn inclusive(v: f64) -> Self {
        ScoreBound {
            value: v,
            exclusive: false,
            is_neg_inf: false,
            is_pos_inf: false,
        }
    }

    pub fn exclusive(v: f64) -> Self {
        ScoreBound {
            value: v,
            exclusive: true,
            is_neg_inf: false,
            is_pos_inf: false,
        }
    }

    /// 解析 Redis 风格的下界字符串（"-inf", "(1.0", "1.0"）
    pub fn parse_min(s: &str) -> Result<Self> {
        parse_score_bound(s, false)
    }

    /// 解析 Redis 风格的上界字符串（"+inf", "(2.0", "2.0"）
    pub fn parse_max(s: &str) -> Result<Self> {
        parse_score_bound(s, true)
    }

    /// 检查 score 是否 ≥ 该下界
    pub fn contains(&self, score: f64) -> bool {
        if self.is_neg_inf {
            return true;
        }
        if self.exclusive {
            score > self.value
        } else {
            score >= self.value
        }
    }

    /// 检查 score 是否 ≤ 该上界
    pub fn contains_upper(&self, score: f64) -> bool {
        if self.is_pos_inf {
            return true;
        }
        if self.exclusive {
            score < self.value
        } else {
            score <= self.value
        }
    }
}

fn parse_score_bound(s: &str, _is_max: bool) -> Result<ScoreBound> {
    match s {
        "+inf" | "+Inf" | "inf" => Ok(ScoreBound::pos_inf()),
        "-inf" | "-Inf" => Ok(ScoreBound::neg_inf()),
        s if s.starts_with('(') => {
            let v: f64 = s[1..].parse().map_err(|_| RedisError::NotFloat)?;
            Ok(ScoreBound::exclusive(v))
        }
        s => {
            let v: f64 = s.parse().map_err(|_| RedisError::NotFloat)?;
            Ok(ScoreBound::inclusive(v))
        }
    }
}

/// 单个逻辑数据库
#[derive(Debug, Default)]
struct LogicalDb {
    data: HashMap<String, RudisObject>,
    expiry: HashMap<String, Instant>,
}

impl LogicalDb {
    fn get(&self, key: &str) -> Option<&RudisObject> {
        if let Some(exp) = self.expiry.get(key) {
            if Instant::now() >= *exp {
                return None;
            }
        }
        self.data.get(key)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut RudisObject> {
        if self.is_expired(key) {
            self.data.remove(key);
            self.expiry.remove(key);
            return None;
        }
        self.data.get_mut(key)
    }

    fn is_expired(&self, key: &str) -> bool {
        if let Some(exp) = self.expiry.get(key) {
            Instant::now() >= *exp
        } else {
            false
        }
    }

    fn remove(&mut self, key: &str) -> Option<RudisObject> {
        self.expiry.remove(key);
        self.data.remove(key)
    }

    fn insert(&mut self, key: String, value: RudisObject) {
        self.expiry.remove(&key);
        self.data.insert(key, value);
    }

    fn set_expiry(&mut self, key: &str, duration: Duration) -> bool {
        if self.data.contains_key(key) {
            self.expiry
                .insert(key.to_owned(), Instant::now() + duration);
            true
        } else {
            false
        }
    }

    fn set_expiry_at(&mut self, key: &str, at: Instant) -> bool {
        if self.data.contains_key(key) {
            self.expiry.insert(key.to_owned(), at);
            true
        } else {
            false
        }
    }

    fn ttl_ms(&self, key: &str) -> Option<i64> {
        if !self.data.contains_key(key) {
            return Some(-2); // 键不存在
        }
        match self.expiry.get(key) {
            None => Some(-1), // 无过期时间
            Some(exp) => {
                let now = Instant::now();
                if now >= *exp {
                    Some(-2)
                } else {
                    let ms = exp.duration_since(now).as_millis() as i64;
                    Some(ms)
                }
            }
        }
    }

    fn persist(&mut self, key: &str) -> bool {
        self.expiry.remove(key).is_some()
    }

    /// 清除已过期的 key（主动淘汰，在后台定期调用）
    fn evict_expired(&mut self) {
        let now = Instant::now();
        let expired: Vec<String> = self
            .expiry
            .iter()
            .filter(|(_, exp)| now >= **exp)
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired {
            self.data.remove(&key);
            self.expiry.remove(&key);
        }
    }
}

/// 完整数据库（包含 16 个逻辑库）
pub struct Database {
    dbs: Vec<LogicalDb>,
}

impl Database {
    pub fn new(db_count: usize) -> Self {
        let mut dbs = Vec::with_capacity(db_count);
        for _ in 0..db_count {
            dbs.push(LogicalDb::default());
        }
        Database { dbs }
    }

    fn db(&self, index: usize) -> Result<&LogicalDb> {
        self.dbs.get(index).ok_or(RedisError::DbIndexOutOfRange)
    }

    fn db_mut(&mut self, index: usize) -> Result<&mut LogicalDb> {
        self.dbs.get_mut(index).ok_or(RedisError::DbIndexOutOfRange)
    }

    // ─────────────────────────────────────────────────────────────
    //  通用 KV 操作
    // ─────────────────────────────────────────────────────────────

    pub fn exists(&self, db: usize, keys: &[String]) -> Result<i64> {
        let ldb = self.db(db)?;
        let count = keys.iter().filter(|k| ldb.get(k).is_some()).count();
        Ok(count as i64)
    }

    pub fn del(&mut self, db: usize, keys: &[String]) -> Result<i64> {
        let ldb = self.db_mut(db)?;
        let count = keys.iter().filter(|k| ldb.remove(k).is_some()).count();
        Ok(count as i64)
    }

    pub fn type_of(&self, db: usize, key: &str) -> Result<&'static str> {
        let ldb = self.db(db)?;
        match ldb.get(key) {
            None => Ok("none"),
            Some(obj) => Ok(obj.type_name()),
        }
    }

    pub fn rename(&mut self, db: usize, src: &str, dst: &str) -> Result<()> {
        let ldb = self.db_mut(db)?;
        let obj = ldb.remove(src).ok_or(RedisError::NoSuchKey)?;
        ldb.insert(dst.to_owned(), obj);
        Ok(())
    }

    pub fn renamenx(&mut self, db: usize, src: &str, dst: &str) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        if ldb.get(dst).is_some() {
            return Ok(false);
        }
        let obj = ldb.remove(src).ok_or(RedisError::NoSuchKey)?;
        ldb.insert(dst.to_owned(), obj);
        Ok(true)
    }

    pub fn keys(&self, db: usize, pattern: &str) -> Result<Vec<String>> {
        let ldb = self.db(db)?;
        let now = Instant::now();
        let matched = ldb
            .data
            .keys()
            .filter(|k| {
                let not_expired = ldb.expiry.get(*k).map(|exp| now < *exp).unwrap_or(true);
                not_expired && glob_match(pattern, k)
            })
            .cloned()
            .collect();
        Ok(matched)
    }

    pub fn dbsize(&self, db: usize) -> Result<usize> {
        let ldb = self.db(db)?;
        let now = Instant::now();
        let size = ldb
            .data
            .keys()
            .filter(|k| ldb.expiry.get(*k).map(|exp| now < *exp).unwrap_or(true))
            .count();
        Ok(size)
    }

    pub fn flushdb(&mut self, db: usize) -> Result<()> {
        let ldb = self.db_mut(db)?;
        ldb.data.clear();
        ldb.expiry.clear();
        Ok(())
    }

    pub fn flushall(&mut self) {
        for ldb in &mut self.dbs {
            ldb.data.clear();
            ldb.expiry.clear();
        }
    }

    pub fn randomkey(&self, db: usize) -> Result<Option<String>> {
        let ldb = self.db(db)?;
        let now = Instant::now();
        let keys: Vec<_> = ldb
            .data
            .keys()
            .filter(|k| ldb.expiry.get(*k).map(|exp| now < *exp).unwrap_or(true))
            .collect();
        if keys.is_empty() {
            return Ok(None);
        }
        use rand::seq::IndexedRandom;
        let key = keys.choose(&mut rand::rng()).cloned().cloned();
        Ok(key)
    }

    // ─────────────────────────────────────────────────────────────
    //  TTL / 过期操作
    // ─────────────────────────────────────────────────────────────

    pub fn expire(&mut self, db: usize, key: &str, secs: i64) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        if secs < 0 {
            // 负数过期时间等同于立即删除
            return Ok(ldb.remove(key).is_some());
        }
        Ok(ldb.set_expiry(key, Duration::from_secs(secs as u64)))
    }

    pub fn pexpire(&mut self, db: usize, key: &str, ms: i64) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        if ms < 0 {
            return Ok(ldb.remove(key).is_some());
        }
        Ok(ldb.set_expiry(key, Duration::from_millis(ms as u64)))
    }

    pub fn expireat(&mut self, db: usize, key: &str, ts_secs: i64) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        let target = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(ts_secs as u64);
        let now_sys = std::time::SystemTime::now();
        if target <= now_sys {
            return Ok(ldb.remove(key).is_some());
        }
        let diff = target.duration_since(now_sys).unwrap_or(Duration::ZERO);
        Ok(ldb.set_expiry_at(key, Instant::now() + diff))
    }

    pub fn pexpireat(&mut self, db: usize, key: &str, ts_ms: i64) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        let target = std::time::SystemTime::UNIX_EPOCH + Duration::from_millis(ts_ms as u64);
        let now_sys = std::time::SystemTime::now();
        if target <= now_sys {
            return Ok(ldb.remove(key).is_some());
        }
        let diff = target.duration_since(now_sys).unwrap_or(Duration::ZERO);
        Ok(ldb.set_expiry_at(key, Instant::now() + diff))
    }

    /// 返回剩余生存时间（毫秒）。-1 永不过期，-2 键不存在
    pub fn pttl(&self, db: usize, key: &str) -> Result<i64> {
        Ok(self.db(db)?.ttl_ms(key).unwrap_or(-2))
    }

    /// 返回剩余生存时间（秒）
    pub fn ttl(&self, db: usize, key: &str) -> Result<i64> {
        let ms = self.pttl(db, key)?;
        if ms >= 0 {
            Ok(ms / 1000)
        } else {
            Ok(ms)
        }
    }

    pub fn persist(&mut self, db: usize, key: &str) -> Result<bool> {
        Ok(self.db_mut(db)?.persist(key))
    }

    /// 后台定期清理过期键
    pub fn evict_expired(&mut self) {
        for ldb in &mut self.dbs {
            ldb.evict_expired();
        }
    }

    // ─────────────────────────────────────────────────────────────
    //  String 操作
    // ─────────────────────────────────────────────────────────────

    pub fn str_get(&self, db: usize, key: &str) -> Result<Option<Vec<u8>>> {
        match self.db(db)?.get(key) {
            None => Ok(None),
            Some(RudisObject::Str(v)) => Ok(Some(v.clone())),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn str_set(
        &mut self,
        db: usize,
        key: String,
        value: Vec<u8>,
        expire: Option<Duration>,
    ) -> Result<()> {
        let ldb = self.db_mut(db)?;
        ldb.insert(key.clone(), RudisObject::Str(value));
        if let Some(dur) = expire {
            ldb.set_expiry(&key, dur);
        }
        Ok(())
    }

    /// SET NX：仅当不存在时设置，返回是否设置成功
    pub fn str_setnx(&mut self, db: usize, key: String, value: Vec<u8>) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        if ldb.get(&key).is_some() {
            return Ok(false);
        }
        ldb.insert(key, RudisObject::Str(value));
        Ok(true)
    }

    /// SET XX：仅当存在时设置，返回是否设置成功
    pub fn str_setxx(
        &mut self,
        db: usize,
        key: String,
        value: Vec<u8>,
        expire: Option<Duration>,
    ) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        if ldb.get(&key).is_none() {
            return Ok(false);
        }
        ldb.insert(key.clone(), RudisObject::Str(value));
        if let Some(dur) = expire {
            ldb.set_expiry(&key, dur);
        }
        Ok(true)
    }

    pub fn str_getset(
        &mut self,
        db: usize,
        key: String,
        value: Vec<u8>,
    ) -> Result<Option<Vec<u8>>> {
        let old = self.str_get(db, &key)?;
        self.str_set(db, key, value, None)?;
        Ok(old)
    }

    pub fn str_getdel(&mut self, db: usize, key: &str) -> Result<Option<Vec<u8>>> {
        let ldb = self.db_mut(db)?;
        match ldb.remove(key) {
            None => Ok(None),
            Some(RudisObject::Str(v)) => Ok(Some(v)),
            Some(other) => {
                // 放回去
                ldb.insert(key.to_owned(), other);
                Err(RedisError::WrongType)
            }
        }
    }

    pub fn str_append(&mut self, db: usize, key: &str, val: &[u8]) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        match ldb
            .data
            .entry(key.to_owned())
            .or_insert(RudisObject::Str(vec![]))
        {
            RudisObject::Str(v) => {
                v.extend_from_slice(val);
                Ok(v.len())
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn str_strlen(&self, db: usize, key: &str) -> Result<usize> {
        match self.db(db)?.get(key) {
            None => Ok(0),
            Some(RudisObject::Str(v)) => Ok(v.len()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn str_incr(&mut self, db: usize, key: &str, delta: i64) -> Result<i64> {
        let ldb = self.db_mut(db)?;
        let entry = ldb
            .data
            .entry(key.to_owned())
            .or_insert(RudisObject::Str(b"0".to_vec()));
        match entry {
            RudisObject::Str(v) => {
                let s = std::str::from_utf8(v).map_err(|_| RedisError::NotInteger)?;
                let n: i64 = s.trim().parse().map_err(|_| RedisError::NotInteger)?;
                let result = n.checked_add(delta).ok_or_else(|| {
                    RedisError::Generic("ERR increment or decrement would overflow".into())
                })?;
                *v = result.to_string().into_bytes();
                Ok(result)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn str_incr_float(&mut self, db: usize, key: &str, delta: f64) -> Result<f64> {
        let ldb = self.db_mut(db)?;
        let entry = ldb
            .data
            .entry(key.to_owned())
            .or_insert(RudisObject::Str(b"0".to_vec()));
        match entry {
            RudisObject::Str(v) => {
                let s = std::str::from_utf8(v).map_err(|_| RedisError::NotFloat)?;
                let n: f64 = s.trim().parse().map_err(|_| RedisError::NotFloat)?;
                let result = n + delta;
                if result.is_nan() || result.is_infinite() {
                    return Err(RedisError::Generic(
                        "ERR increment would produce NaN or Infinity".into(),
                    ));
                }
                *v = format_float(result).into_bytes();
                Ok(result)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn str_getrange(&self, db: usize, key: &str, start: i64, end: i64) -> Result<Vec<u8>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![]),
            Some(RudisObject::Str(v)) => {
                let len = v.len() as i64;
                let s = normalize_index(start, len);
                let e = normalize_index(end, len).min((len - 1).max(0) as usize);
                if s > e || s >= v.len() {
                    return Ok(vec![]);
                }
                Ok(v[s..=e.min(v.len() - 1)].to_vec())
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn str_setrange(
        &mut self,
        db: usize,
        key: &str,
        offset: usize,
        val: &[u8],
    ) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        let v = match ldb
            .data
            .entry(key.to_owned())
            .or_insert(RudisObject::Str(vec![]))
        {
            RudisObject::Str(v) => v,
            _ => return Err(RedisError::WrongType),
        };
        let needed = offset + val.len();
        if v.len() < needed {
            v.resize(needed, 0);
        }
        v[offset..offset + val.len()].copy_from_slice(val);
        Ok(v.len())
    }

    pub fn mset(&mut self, db: usize, pairs: Vec<(String, Vec<u8>)>) -> Result<()> {
        let ldb = self.db_mut(db)?;
        for (k, v) in pairs {
            ldb.insert(k, RudisObject::Str(v));
        }
        Ok(())
    }

    pub fn mget(&self, db: usize, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let ldb = self.db(db)?;
        let result = keys
            .iter()
            .map(|k| match ldb.get(k) {
                Some(RudisObject::Str(v)) => Some(v.clone()),
                _ => None,
            })
            .collect();
        Ok(result)
    }

    pub fn msetnx(&mut self, db: usize, pairs: Vec<(String, Vec<u8>)>) -> Result<bool> {
        let ldb = self.db_mut(db)?;
        for (k, _) in &pairs {
            if ldb.get(k).is_some() {
                return Ok(false);
            }
        }
        for (k, v) in pairs {
            ldb.insert(k, RudisObject::Str(v));
        }
        Ok(true)
    }

    // ─────────────────────────────────────────────────────────────
    //  List 操作
    // ─────────────────────────────────────────────────────────────

    fn get_list<'a>(&'a self, db: usize, key: &str) -> Result<Option<&'a VecDeque<Vec<u8>>>> {
        match self.db(db)?.get(key) {
            None => Ok(None),
            Some(RudisObject::List(l)) => Ok(Some(l)),
            _ => Err(RedisError::WrongType),
        }
    }

    fn get_list_mut<'a>(
        &'a mut self,
        db: usize,
        key: &str,
    ) -> Result<Option<&'a mut VecDeque<Vec<u8>>>> {
        match self.db_mut(db)?.get_mut(key) {
            None => Ok(None),
            Some(RudisObject::List(l)) => Ok(Some(l)),
            _ => Err(RedisError::WrongType),
        }
    }

    fn get_or_create_list<'a>(
        &'a mut self,
        db: usize,
        key: &str,
    ) -> Result<&'a mut VecDeque<Vec<u8>>> {
        let ldb = self.db_mut(db)?;
        let entry = ldb
            .data
            .entry(key.to_owned())
            .or_insert_with(|| RudisObject::List(VecDeque::new()));
        match entry {
            RudisObject::List(l) => Ok(l),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn lpush(&mut self, db: usize, key: &str, values: Vec<Vec<u8>>) -> Result<usize> {
        let list = self.get_or_create_list(db, key)?;
        for v in values {
            list.push_front(v);
        }
        Ok(list.len())
    }

    pub fn rpush(&mut self, db: usize, key: &str, values: Vec<Vec<u8>>) -> Result<usize> {
        let list = self.get_or_create_list(db, key)?;
        for v in values {
            list.push_back(v);
        }
        Ok(list.len())
    }

    pub fn lpushx(&mut self, db: usize, key: &str, values: Vec<Vec<u8>>) -> Result<usize> {
        match self.get_list_mut(db, key)? {
            None => Ok(0),
            Some(list) => {
                for v in values {
                    list.push_front(v);
                }
                Ok(list.len())
            }
        }
    }

    pub fn rpushx(&mut self, db: usize, key: &str, values: Vec<Vec<u8>>) -> Result<usize> {
        match self.get_list_mut(db, key)? {
            None => Ok(0),
            Some(list) => {
                for v in values {
                    list.push_back(v);
                }
                Ok(list.len())
            }
        }
    }

    pub fn lpop(&mut self, db: usize, key: &str, count: usize) -> Result<Vec<Vec<u8>>> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(vec![]),
            Some(RudisObject::List(l)) => {
                let mut result = Vec::with_capacity(count);
                for _ in 0..count {
                    match l.pop_front() {
                        Some(v) => result.push(v),
                        None => break,
                    }
                }
                if l.is_empty() {
                    ldb.remove(key);
                }
                Ok(result)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn rpop(&mut self, db: usize, key: &str, count: usize) -> Result<Vec<Vec<u8>>> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(vec![]),
            Some(RudisObject::List(l)) => {
                let mut result = Vec::with_capacity(count);
                for _ in 0..count {
                    match l.pop_back() {
                        Some(v) => result.push(v),
                        None => break,
                    }
                }
                if l.is_empty() {
                    ldb.remove(key);
                }
                Ok(result)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn llen(&self, db: usize, key: &str) -> Result<usize> {
        match self.get_list(db, key)? {
            None => Ok(0),
            Some(l) => Ok(l.len()),
        }
    }

    pub fn lrange(&self, db: usize, key: &str, start: i64, stop: i64) -> Result<Vec<Vec<u8>>> {
        match self.get_list(db, key)? {
            None => Ok(vec![]),
            Some(l) => {
                let len = l.len() as i64;
                let s = normalize_index(start, len);
                let e = normalize_index(stop, len).min((len - 1).max(0) as usize);
                if s > l.len() || (len > 0 && s > e) {
                    return Ok(vec![]);
                }
                Ok(l.iter()
                    .skip(s)
                    .take(e.saturating_sub(s) + 1)
                    .cloned()
                    .collect())
            }
        }
    }

    pub fn lindex(&self, db: usize, key: &str, index: i64) -> Result<Option<Vec<u8>>> {
        match self.get_list(db, key)? {
            None => Ok(None),
            Some(l) => {
                let len = l.len() as i64;
                let i = normalize_index(index, len);
                Ok(l.get(i).cloned())
            }
        }
    }

    pub fn lset(&mut self, db: usize, key: &str, index: i64, value: Vec<u8>) -> Result<()> {
        match self.get_list_mut(db, key)? {
            None => Err(RedisError::NoSuchKey),
            Some(l) => {
                let len = l.len() as i64;
                let i = normalize_index(index, len);
                match l.get_mut(i) {
                    Some(elem) => {
                        *elem = value;
                        Ok(())
                    }
                    None => Err(RedisError::IndexOutOfRange),
                }
            }
        }
    }

    pub fn linsert(
        &mut self,
        db: usize,
        key: &str,
        before: bool,
        pivot: &[u8],
        value: Vec<u8>,
    ) -> Result<i64> {
        match self.get_list_mut(db, key)? {
            None => Ok(-1),
            Some(l) => {
                if let Some(pos) = l.iter().position(|e| e.as_slice() == pivot) {
                    let insert_pos = if before { pos } else { pos + 1 };
                    l.insert(insert_pos, value);
                    Ok(l.len() as i64)
                } else {
                    Ok(-1)
                }
            }
        }
    }

    pub fn lrem(&mut self, db: usize, key: &str, count: i64, value: &[u8]) -> Result<usize> {
        match self.get_list_mut(db, key)? {
            None => Ok(0),
            Some(l) => {
                let mut removed = 0usize;
                if count == 0 {
                    l.retain(|e| {
                        if e.as_slice() == value {
                            removed += 1;
                            false
                        } else {
                            true
                        }
                    });
                } else if count > 0 {
                    let mut remaining = count as usize;
                    l.retain(|e| {
                        if remaining > 0 && e.as_slice() == value {
                            remaining -= 1;
                            removed += 1;
                            false
                        } else {
                            true
                        }
                    });
                } else {
                    // count < 0：从尾部删除
                    let mut remaining = count.unsigned_abs() as usize;
                    let len = l.len();
                    let mut keep: Vec<bool> = vec![true; len];
                    for i in (0..len).rev() {
                        if remaining > 0 && l[i].as_slice() == value {
                            keep[i] = false;
                            remaining -= 1;
                            removed += 1;
                        }
                    }
                    let mut i = 0;
                    l.retain(|_| {
                        let r = keep[i];
                        i += 1;
                        r
                    });
                }
                Ok(removed)
            }
        }
    }

    pub fn ltrim(&mut self, db: usize, key: &str, start: i64, stop: i64) -> Result<()> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(()),
            Some(RudisObject::List(l)) => {
                let len = l.len() as i64;
                let s = normalize_index(start, len);
                let e = normalize_index(stop, len).min((len - 1).max(0) as usize);
                let new_list: VecDeque<Vec<u8>> = if s > l.len() || (len > 0 && s > e) {
                    VecDeque::new()
                } else {
                    l.iter().skip(s).take(e - s + 1).cloned().collect()
                };
                *l = new_list;
                if l.is_empty() {
                    ldb.remove(key);
                }
                Ok(())
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn lmove(
        &mut self,
        db: usize,
        src: &str,
        dst: &str,
        src_left: bool,
        dst_left: bool,
    ) -> Result<Option<Vec<u8>>> {
        if src == dst {
            let ldb = self.db_mut(db)?;
            match ldb.data.get_mut(src) {
                None => return Ok(None),
                Some(RudisObject::List(l)) => {
                    if l.is_empty() {
                        return Ok(None);
                    }
                    let elem = if src_left {
                        l.pop_front().unwrap()
                    } else {
                        l.pop_back().unwrap()
                    };
                    if dst_left {
                        l.push_front(elem.clone());
                    } else {
                        l.push_back(elem.clone());
                    }
                    return Ok(Some(elem));
                }
                _ => return Err(RedisError::WrongType),
            }
        }

        let src_key = src.to_owned();
        let dst_key = dst.to_owned();

        // 先从源列表取元素
        let elem = {
            let ldb = self.db_mut(db)?;
            match ldb.data.get_mut(&src_key) {
                None => return Ok(None),
                Some(RudisObject::List(l)) => {
                    let e = if src_left {
                        l.pop_front()
                    } else {
                        l.pop_back()
                    };
                    if l.is_empty() {
                        ldb.remove(&src_key);
                    }
                    e
                }
                _ => return Err(RedisError::WrongType),
            }
        };

        if let Some(e) = elem {
            let list = self.get_or_create_list(db, &dst_key)?;
            if dst_left {
                list.push_front(e.clone());
            } else {
                list.push_back(e.clone());
            }
            Ok(Some(e))
        } else {
            Ok(None)
        }
    }

    // ─────────────────────────────────────────────────────────────
    //  Hash 操作
    // ─────────────────────────────────────────────────────────────

    fn get_or_create_hash<'a>(
        &'a mut self,
        db: usize,
        key: &str,
    ) -> Result<&'a mut HashMap<String, Vec<u8>>> {
        let ldb = self.db_mut(db)?;
        let entry = ldb
            .data
            .entry(key.to_owned())
            .or_insert_with(|| RudisObject::Hash(HashMap::new()));
        match entry {
            RudisObject::Hash(h) => Ok(h),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hset(&mut self, db: usize, key: &str, fields: Vec<(String, Vec<u8>)>) -> Result<usize> {
        let hash = self.get_or_create_hash(db, key)?;
        let mut new_fields = 0;
        for (f, v) in fields {
            if hash.insert(f, v).is_none() {
                new_fields += 1;
            }
        }
        Ok(new_fields)
    }

    pub fn hsetnx(&mut self, db: usize, key: &str, field: &str, value: Vec<u8>) -> Result<bool> {
        let hash = self.get_or_create_hash(db, key)?;
        if hash.contains_key(field) {
            return Ok(false);
        }
        hash.insert(field.to_owned(), value);
        Ok(true)
    }

    pub fn hget(&self, db: usize, key: &str, field: &str) -> Result<Option<Vec<u8>>> {
        match self.db(db)?.get(key) {
            None => Ok(None),
            Some(RudisObject::Hash(h)) => Ok(h.get(field).cloned()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hmget(&self, db: usize, key: &str, fields: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![None; fields.len()]),
            Some(RudisObject::Hash(h)) => Ok(fields.iter().map(|f| h.get(f).cloned()).collect()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hdel(&mut self, db: usize, key: &str, fields: &[String]) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(0),
            Some(RudisObject::Hash(h)) => {
                let count = fields.iter().filter(|f| h.remove(*f).is_some()).count();
                if h.is_empty() {
                    ldb.remove(key);
                }
                Ok(count)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hexists(&self, db: usize, key: &str, field: &str) -> Result<bool> {
        match self.db(db)?.get(key) {
            None => Ok(false),
            Some(RudisObject::Hash(h)) => Ok(h.contains_key(field)),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hgetall(&self, db: usize, key: &str) -> Result<Vec<(String, Vec<u8>)>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![]),
            Some(RudisObject::Hash(h)) => {
                Ok(h.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hkeys(&self, db: usize, key: &str) -> Result<Vec<String>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![]),
            Some(RudisObject::Hash(h)) => Ok(h.keys().cloned().collect()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hvals(&self, db: usize, key: &str) -> Result<Vec<Vec<u8>>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![]),
            Some(RudisObject::Hash(h)) => Ok(h.values().cloned().collect()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hlen(&self, db: usize, key: &str) -> Result<usize> {
        match self.db(db)?.get(key) {
            None => Ok(0),
            Some(RudisObject::Hash(h)) => Ok(h.len()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn hincrby(&mut self, db: usize, key: &str, field: &str, delta: i64) -> Result<i64> {
        let hash = self.get_or_create_hash(db, key)?;
        let val = hash
            .entry(field.to_owned())
            .or_insert_with(|| b"0".to_vec());
        let s = std::str::from_utf8(val).map_err(|_| RedisError::NotInteger)?;
        let n: i64 = s.trim().parse().map_err(|_| RedisError::NotInteger)?;
        let result = n.checked_add(delta).ok_or_else(|| {
            RedisError::Generic("ERR increment or decrement would overflow".into())
        })?;
        *val = result.to_string().into_bytes();
        Ok(result)
    }

    pub fn hincrbyfloat(&mut self, db: usize, key: &str, field: &str, delta: f64) -> Result<f64> {
        let hash = self.get_or_create_hash(db, key)?;
        let val = hash
            .entry(field.to_owned())
            .or_insert_with(|| b"0".to_vec());
        let s = std::str::from_utf8(val).map_err(|_| RedisError::NotFloat)?;
        let n: f64 = s.trim().parse().map_err(|_| RedisError::NotFloat)?;
        let result = n + delta;
        if result.is_nan() {
            return Err(RedisError::Generic(
                "ERR resulting score is not a number (NaN)".into(),
            ));
        }
        *val = format_float(result).into_bytes();
        Ok(result)
    }

    // ─────────────────────────────────────────────────────────────
    //  Set 操作
    // ─────────────────────────────────────────────────────────────

    fn get_or_create_set<'a>(
        &'a mut self,
        db: usize,
        key: &str,
    ) -> Result<&'a mut HashSet<String>> {
        let ldb = self.db_mut(db)?;
        let entry = ldb
            .data
            .entry(key.to_owned())
            .or_insert_with(|| RudisObject::Set(HashSet::new()));
        match entry {
            RudisObject::Set(s) => Ok(s),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn sadd(&mut self, db: usize, key: &str, members: Vec<String>) -> Result<usize> {
        let set = self.get_or_create_set(db, key)?;
        let count = members
            .into_iter()
            .filter(|m| set.insert(m.clone()))
            .count();
        Ok(count)
    }

    pub fn srem(&mut self, db: usize, key: &str, members: &[String]) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(0),
            Some(RudisObject::Set(s)) => {
                let count = members.iter().filter(|m| s.remove(*m)).count();
                if s.is_empty() {
                    ldb.remove(key);
                }
                Ok(count)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn smembers(&self, db: usize, key: &str) -> Result<Vec<String>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![]),
            Some(RudisObject::Set(s)) => Ok(s.iter().cloned().collect()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn sismember(&self, db: usize, key: &str, member: &str) -> Result<bool> {
        match self.db(db)?.get(key) {
            None => Ok(false),
            Some(RudisObject::Set(s)) => Ok(s.contains(member)),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn smismember(&self, db: usize, key: &str, members: &[String]) -> Result<Vec<bool>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![false; members.len()]),
            Some(RudisObject::Set(s)) => Ok(members.iter().map(|m| s.contains(m)).collect()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn scard(&self, db: usize, key: &str) -> Result<usize> {
        match self.db(db)?.get(key) {
            None => Ok(0),
            Some(RudisObject::Set(s)) => Ok(s.len()),
            _ => Err(RedisError::WrongType),
        }
    }

    fn get_set_clone(&self, db: usize, key: &str) -> Result<HashSet<String>> {
        match self.db(db)?.get(key) {
            None => Ok(HashSet::new()),
            Some(RudisObject::Set(s)) => Ok(s.clone()),
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn sunion(&self, db: usize, keys: &[String]) -> Result<Vec<String>> {
        let mut result: HashSet<String> = HashSet::new();
        for key in keys {
            result.extend(self.get_set_clone(db, key)?);
        }
        Ok(result.into_iter().collect())
    }

    pub fn sinter(&self, db: usize, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }
        let mut sets: Vec<HashSet<String>> = Vec::new();
        for key in keys {
            sets.push(self.get_set_clone(db, key)?);
        }
        let (first, rest) = sets.split_first().unwrap();
        let result: Vec<String> = first
            .iter()
            .filter(|m| rest.iter().all(|s| s.contains(*m)))
            .cloned()
            .collect();
        Ok(result)
    }

    pub fn sdiff(&self, db: usize, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }
        let first = self.get_set_clone(db, &keys[0])?;
        let rest_sets: Vec<HashSet<String>> = keys[1..]
            .iter()
            .map(|k| self.get_set_clone(db, k))
            .collect::<Result<_>>()?;
        let result: Vec<String> = first
            .into_iter()
            .filter(|m| !rest_sets.iter().any(|s| s.contains(m)))
            .collect();
        Ok(result)
    }

    pub fn sunionstore(&mut self, db: usize, dst: &str, keys: &[String]) -> Result<usize> {
        let result = self.sunion(db, keys)?;
        let count = result.len();
        let ldb = self.db_mut(db)?;
        let set: HashSet<String> = result.into_iter().collect();
        ldb.insert(dst.to_owned(), RudisObject::Set(set));
        Ok(count)
    }

    pub fn sinterstore(&mut self, db: usize, dst: &str, keys: &[String]) -> Result<usize> {
        let result = self.sinter(db, keys)?;
        let count = result.len();
        let ldb = self.db_mut(db)?;
        let set: HashSet<String> = result.into_iter().collect();
        ldb.insert(dst.to_owned(), RudisObject::Set(set));
        Ok(count)
    }

    pub fn sdiffstore(&mut self, db: usize, dst: &str, keys: &[String]) -> Result<usize> {
        let result = self.sdiff(db, keys)?;
        let count = result.len();
        let ldb = self.db_mut(db)?;
        let set: HashSet<String> = result.into_iter().collect();
        ldb.insert(dst.to_owned(), RudisObject::Set(set));
        Ok(count)
    }

    pub fn spop(&mut self, db: usize, key: &str, count: usize) -> Result<Vec<String>> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(vec![]),
            Some(RudisObject::Set(s)) => {
                use rand::seq::IteratorRandom;
                let chosen: Vec<String> = s
                    .iter()
                    .choose_multiple(&mut rand::rng(), count)
                    .into_iter()
                    .cloned()
                    .collect();
                for m in &chosen {
                    s.remove(m);
                }
                if s.is_empty() {
                    ldb.remove(key);
                }
                Ok(chosen)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn srandmember(&self, db: usize, key: &str, count: i64) -> Result<Vec<String>> {
        match self.db(db)?.get(key) {
            None => Ok(vec![]),
            Some(RudisObject::Set(s)) => {
                use rand::seq::IteratorRandom;
                if count >= 0 {
                    Ok(s.iter()
                        .choose_multiple(&mut rand::rng(), count as usize)
                        .into_iter()
                        .cloned()
                        .collect())
                } else {
                    // 负数：允许重复
                    let abs = count.unsigned_abs() as usize;
                    let members: Vec<_> = s.iter().collect();
                    if members.is_empty() {
                        return Ok(vec![]);
                    }
                    use rand::Rng;
                    let mut rng = rand::rng();
                    Ok((0..abs)
                        .map(|_| members[rng.random_range(0..members.len())].clone())
                        .collect())
                }
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn smove(&mut self, db: usize, src: &str, dst: &str, member: &str) -> Result<bool> {
        // Step 1: 从源集合移除成员（此块结束后释放可变借用）
        let src_became_empty = {
            let ldb = self.db_mut(db)?;
            match ldb.data.get_mut(src) {
                None => return Ok(false),
                Some(RudisObject::Set(s)) => {
                    if !s.remove(member) {
                        return Ok(false);
                    }
                    s.is_empty()
                }
                _ => return Err(RedisError::WrongType),
            }
        };
        if src_became_empty {
            self.db_mut(db)?.remove(src);
        }
        // Step 2: 将成员加入目标集合
        let dst_set = self.get_or_create_set(db, dst)?;
        dst_set.insert(member.to_owned());
        Ok(true)
    }

    // ─────────────────────────────────────────────────────────────
    //  ZSet 操作
    // ─────────────────────────────────────────────────────────────

    fn get_or_create_zset<'a>(&'a mut self, db: usize, key: &str) -> Result<&'a mut ZSetInner> {
        let ldb = self.db_mut(db)?;
        let entry = ldb
            .data
            .entry(key.to_owned())
            .or_insert_with(|| RudisObject::ZSet(ZSetInner::new()));
        match entry {
            RudisObject::ZSet(z) => Ok(z),
            _ => Err(RedisError::WrongType),
        }
    }

    fn get_zset<'a>(&'a self, db: usize, key: &str) -> Result<Option<&'a ZSetInner>> {
        match self.db(db)?.get(key) {
            None => Ok(None),
            Some(RudisObject::ZSet(z)) => Ok(Some(z)),
            _ => Err(RedisError::WrongType),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn zadd(
        &mut self,
        db: usize,
        key: &str,
        members: Vec<(f64, String)>,
        nx: bool,
        xx: bool,
        gt: bool,
        lt: bool,
        ch: bool,
    ) -> Result<i64> {
        let zset = self.get_or_create_zset(db, key)?;
        let mut count = 0i64;
        for (score, member) in members {
            if score.is_nan() {
                return Err(RedisError::ScoreNaN);
            }
            if nx && zset.scores.contains_key(&member) {
                continue;
            }
            if xx && !zset.scores.contains_key(&member) {
                continue;
            }
            if gt {
                if let Some(&old) = zset.scores.get(&member) {
                    if score <= old {
                        continue;
                    }
                }
            }
            if lt {
                if let Some(&old) = zset.scores.get(&member) {
                    if score >= old {
                        continue;
                    }
                }
            }
            let is_new = zset.add(member, score);
            if ch || is_new {
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn zrem(&mut self, db: usize, key: &str, members: &[String]) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(0),
            Some(RudisObject::ZSet(z)) => {
                let count = members.iter().filter(|m| z.remove(m)).count();
                if z.is_empty() {
                    ldb.remove(key);
                }
                Ok(count)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn zscore(&self, db: usize, key: &str, member: &str) -> Result<Option<f64>> {
        match self.get_zset(db, key)? {
            None => Ok(None),
            Some(z) => Ok(z.score(member)),
        }
    }

    pub fn zmscore(&self, db: usize, key: &str, members: &[String]) -> Result<Vec<Option<f64>>> {
        match self.get_zset(db, key)? {
            None => Ok(vec![None; members.len()]),
            Some(z) => Ok(members.iter().map(|m| z.score(m)).collect()),
        }
    }

    pub fn zincrby(&mut self, db: usize, key: &str, delta: f64, member: &str) -> Result<f64> {
        let zset = self.get_or_create_zset(db, key)?;
        let old = zset.scores.get(member).copied().unwrap_or(0.0);
        let new_score = old + delta;
        if new_score.is_nan() {
            return Err(RedisError::ScoreNaN);
        }
        zset.add(member.to_owned(), new_score);
        Ok(new_score)
    }

    pub fn zrank(&self, db: usize, key: &str, member: &str) -> Result<Option<usize>> {
        match self.get_zset(db, key)? {
            None => Ok(None),
            Some(z) => Ok(z.rank(member)),
        }
    }

    pub fn zrevrank(&self, db: usize, key: &str, member: &str) -> Result<Option<usize>> {
        match self.get_zset(db, key)? {
            None => Ok(None),
            Some(z) => Ok(z.revrank(member)),
        }
    }

    pub fn zcard(&self, db: usize, key: &str) -> Result<usize> {
        match self.get_zset(db, key)? {
            None => Ok(0),
            Some(z) => Ok(z.len()),
        }
    }

    pub fn zcount(&self, db: usize, key: &str, min: ScoreBound, max: ScoreBound) -> Result<usize> {
        match self.get_zset(db, key)? {
            None => Ok(0),
            Some(z) => Ok(z.count_by_score(min, max)),
        }
    }

    pub fn zrange(
        &self,
        db: usize,
        key: &str,
        start: i64,
        stop: i64,
        rev: bool,
        withscores: bool,
    ) -> Result<Vec<(String, Option<f64>)>> {
        match self.get_zset(db, key)? {
            None => Ok(vec![]),
            Some(z) => {
                let mut result = z.range_by_index(start, stop, withscores);
                if rev {
                    result.reverse();
                }
                Ok(result)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn zrangebyscore(
        &self,
        db: usize,
        key: &str,
        min: ScoreBound,
        max: ScoreBound,
        withscores: bool,
        offset: usize,
        count: Option<usize>,
    ) -> Result<Vec<(String, Option<f64>)>> {
        match self.get_zset(db, key)? {
            None => Ok(vec![]),
            Some(z) => Ok(z.range_by_score(min, max, withscores, offset, count)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn zrevrangebyscore(
        &self,
        db: usize,
        key: &str,
        max: ScoreBound,
        min: ScoreBound,
        withscores: bool,
        offset: usize,
        count: Option<usize>,
    ) -> Result<Vec<(String, Option<f64>)>> {
        match self.get_zset(db, key)? {
            None => Ok(vec![]),
            Some(z) => Ok(z.revrange_by_score(max, min, withscores, offset, count)),
        }
    }

    pub fn zremrangebyscore(
        &mut self,
        db: usize,
        key: &str,
        min: ScoreBound,
        max: ScoreBound,
    ) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(0),
            Some(RudisObject::ZSet(z)) => {
                let count = z.remove_by_score(min, max);
                if z.is_empty() {
                    ldb.remove(key);
                }
                Ok(count)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn zremrangebyrank(
        &mut self,
        db: usize,
        key: &str,
        start: i64,
        stop: i64,
    ) -> Result<usize> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(0),
            Some(RudisObject::ZSet(z)) => {
                let count = z.remove_by_rank(start, stop);
                if z.is_empty() {
                    ldb.remove(key);
                }
                Ok(count)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn zpopmin(&mut self, db: usize, key: &str, count: usize) -> Result<Vec<(String, f64)>> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(vec![]),
            Some(RudisObject::ZSet(z)) => {
                let result = z.pop_min(count);
                if z.is_empty() {
                    ldb.remove(key);
                }
                Ok(result)
            }
            _ => Err(RedisError::WrongType),
        }
    }

    pub fn zpopmax(&mut self, db: usize, key: &str, count: usize) -> Result<Vec<(String, f64)>> {
        let ldb = self.db_mut(db)?;
        match ldb.data.get_mut(key) {
            None => Ok(vec![]),
            Some(RudisObject::ZSet(z)) => {
                let result = z.pop_max(count);
                if z.is_empty() {
                    ldb.remove(key);
                }
                Ok(result)
            }
            _ => Err(RedisError::WrongType),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  工具函数
// ─────────────────────────────────────────────────────────────────────────────

/// 将可能为负数的 Redis 下标转换为实际数组下标
pub fn normalize_index(index: i64, len: i64) -> usize {
    if len == 0 {
        return 0;
    }
    if index < 0 {
        (len + index).max(0) as usize
    } else {
        index as usize
    }
}

/// 以类似 Redis 的格式打印浮点数（去掉不必要的尾零）
pub fn format_float(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        // 最多 17 位有效数字
        let s = format!("{:.17}", f);
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// 简单的 glob 模式匹配（支持 `*` 和 `?`，不支持字符类）
pub fn glob_match(pattern: &str, s: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), s.as_bytes())
}

fn glob_match_bytes(pat: &[u8], s: &[u8]) -> bool {
    match (pat.first(), s.first()) {
        (None, None) => true,
        (Some(&b'*'), _) => {
            glob_match_bytes(&pat[1..], s) || (!s.is_empty() && glob_match_bytes(pat, &s[1..]))
        }
        (Some(&b'?'), Some(_)) => glob_match_bytes(&pat[1..], &s[1..]),
        (Some(p), Some(c)) if p == c => glob_match_bytes(&pat[1..], &s[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db() -> Database {
        Database::new(16)
    }

    #[test]
    fn test_string_set_get() {
        let mut db = make_db();
        db.str_set(0, "key".into(), b"value".to_vec(), None)
            .unwrap();
        assert_eq!(db.str_get(0, "key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_string_incr() {
        let mut db = make_db();
        db.str_set(0, "n".into(), b"10".to_vec(), None).unwrap();
        assert_eq!(db.str_incr(0, "n", 5).unwrap(), 15);
    }

    #[test]
    fn test_list_push_pop() {
        let mut db = make_db();
        db.lpush(0, "list", vec![b"a".to_vec(), b"b".to_vec()])
            .unwrap();
        assert_eq!(db.llen(0, "list").unwrap(), 2);
        let popped = db.lpop(0, "list", 1).unwrap();
        assert_eq!(popped, vec![b"b".to_vec()]);
    }

    #[test]
    fn test_hash_hset_hget() {
        let mut db = make_db();
        db.hset(0, "h", vec![("f".into(), b"v".to_vec())]).unwrap();
        assert_eq!(db.hget(0, "h", "f").unwrap(), Some(b"v".to_vec()));
    }

    #[test]
    fn test_set_sadd_smembers() {
        let mut db = make_db();
        db.sadd(0, "s", vec!["a".into(), "b".into(), "c".into()])
            .unwrap();
        let mut members = db.smembers(0, "s").unwrap();
        members.sort();
        assert_eq!(members, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_zset_zadd_zscore() {
        let mut db = make_db();
        db.zadd(
            0,
            "z",
            vec![(1.0, "a".into()), (2.0, "b".into())],
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(db.zscore(0, "z", "a").unwrap(), Some(1.0));
        assert_eq!(db.zrank(0, "z", "b").unwrap(), Some(1));
    }

    #[test]
    fn test_ttl_expire() {
        let mut db = make_db();
        db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
        db.expire(0, "k", 100).unwrap();
        let ttl = db.ttl(0, "k").unwrap();
        assert!(ttl > 0 && ttl <= 100);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("h?llo", "hello"));
        assert!(glob_match("h*llo", "heeello"));
        assert!(!glob_match("h?llo", "hllo"));
        assert!(glob_match("*", "anything"));
    }
}
