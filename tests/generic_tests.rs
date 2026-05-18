/// 通用 Key 操作集成测试（TTL、DEL、EXISTS、TYPE、RENAME、KEYS 等）
use mini_rudis::db::Database;
use std::time::Duration;

fn db() -> Database {
    Database::new(16)
}

#[test]
fn test_del_single_key() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    let deleted = db.del(0, &["k".into()]).unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(db.str_get(0, "k").unwrap(), None);
}

#[test]
fn test_del_multiple_keys() {
    let mut db = db();
    db.str_set(0, "a".into(), b"1".to_vec(), None).unwrap();
    db.str_set(0, "b".into(), b"2".to_vec(), None).unwrap();
    let deleted = db
        .del(0, &["a".into(), "b".into(), "missing".into()])
        .unwrap();
    assert_eq!(deleted, 2);
}

#[test]
fn test_exists() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    assert_eq!(db.exists(0, &["k".into()]).unwrap(), 1);
    assert_eq!(db.exists(0, &["k".into(), "k".into()]).unwrap(), 2); // 同一键计多次
    assert_eq!(db.exists(0, &["missing".into()]).unwrap(), 0);
}

#[test]
fn test_type_of() {
    let mut db = db();
    db.str_set(0, "str".into(), b"v".to_vec(), None).unwrap();
    db.lpush(0, "list", vec![b"v".to_vec()]).unwrap();
    db.hset(0, "hash", vec![("f".into(), b"v".to_vec())])
        .unwrap();
    db.sadd(0, "set", vec!["m".into()]).unwrap();
    db.zadd(
        0,
        "zset",
        vec![(1.0, "m".into())],
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();

    assert_eq!(db.type_of(0, "str").unwrap(), "string");
    assert_eq!(db.type_of(0, "list").unwrap(), "list");
    assert_eq!(db.type_of(0, "hash").unwrap(), "hash");
    assert_eq!(db.type_of(0, "set").unwrap(), "set");
    assert_eq!(db.type_of(0, "zset").unwrap(), "zset");
    assert_eq!(db.type_of(0, "none").unwrap(), "none");
}

#[test]
fn test_rename() {
    let mut db = db();
    db.str_set(0, "old".into(), b"value".to_vec(), None)
        .unwrap();
    db.rename(0, "old", "new").unwrap();
    assert_eq!(db.str_get(0, "old").unwrap(), None);
    assert_eq!(db.str_get(0, "new").unwrap(), Some(b"value".to_vec()));
}

#[test]
fn test_rename_nonexistent_fails() {
    let mut db = db();
    assert!(db.rename(0, "missing", "new").is_err());
}

#[test]
fn test_renamenx() {
    let mut db = db();
    db.str_set(0, "src".into(), b"v".to_vec(), None).unwrap();
    // dst 不存在，应成功
    assert!(db.renamenx(0, "src", "dst").unwrap());
    // src 现在不存在
    db.str_set(0, "src2".into(), b"v2".to_vec(), None).unwrap();
    // dst 已存在，应失败
    assert!(!db.renamenx(0, "src2", "dst").unwrap());
}

#[test]
fn test_expire_and_ttl() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    db.expire(0, "k", 100).unwrap();
    let ttl = db.ttl(0, "k").unwrap();
    assert!(ttl > 0 && ttl <= 100);
    let pttl = db.pttl(0, "k").unwrap();
    assert!(pttl > 0 && pttl <= 100_000);
}

#[test]
fn test_ttl_no_expiry_returns_minus_one() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    assert_eq!(db.ttl(0, "k").unwrap(), -1);
}

#[test]
fn test_ttl_missing_key_returns_minus_two() {
    let db = db();
    assert_eq!(db.ttl(0, "missing").unwrap(), -2);
}

#[test]
fn test_persist() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    db.expire(0, "k", 100).unwrap();
    assert!(db.persist(0, "k").unwrap());
    assert_eq!(db.ttl(0, "k").unwrap(), -1);
}

#[test]
fn test_keys_pattern() {
    let mut db = db();
    db.str_set(0, "user:1".into(), b"a".to_vec(), None).unwrap();
    db.str_set(0, "user:2".into(), b"b".to_vec(), None).unwrap();
    db.str_set(0, "post:1".into(), b"c".to_vec(), None).unwrap();
    let mut keys = db.keys(0, "user:*").unwrap();
    keys.sort();
    assert_eq!(keys, vec!["user:1", "user:2"]);
    let all = db.keys(0, "*").unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_dbsize() {
    let mut db = db();
    db.str_set(0, "a".into(), b"1".to_vec(), None).unwrap();
    db.str_set(0, "b".into(), b"2".to_vec(), None).unwrap();
    assert_eq!(db.dbsize(0).unwrap(), 2);
}

#[test]
fn test_flushdb() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    db.flushdb(0).unwrap();
    assert_eq!(db.dbsize(0).unwrap(), 0);
}

#[test]
fn test_select_different_databases() {
    let mut db = db();
    // db 0 和 db 1 是相互独立的
    db.str_set(0, "k".into(), b"from_db0".to_vec(), None)
        .unwrap();
    assert_eq!(db.str_get(0, "k").unwrap(), Some(b"from_db0".to_vec()));
    assert_eq!(db.str_get(1, "k").unwrap(), None);
    db.str_set(1, "k".into(), b"from_db1".to_vec(), None)
        .unwrap();
    assert_eq!(db.str_get(1, "k").unwrap(), Some(b"from_db1".to_vec()));
}

#[test]
fn test_key_expires_lazily() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), Some(Duration::from_millis(1)))
        .unwrap();
    std::thread::sleep(Duration::from_millis(10));
    // 惰性删除：此时访问应返回 None
    assert_eq!(db.str_get(0, "k").unwrap(), None);
}

#[test]
fn test_msetnx_atomic() {
    let mut db = db();
    // 其中一个 key 已存在，整个操作应失败
    db.str_set(0, "existing".into(), b"v".to_vec(), None)
        .unwrap();
    let result = db
        .msetnx(
            0,
            vec![
                ("existing".into(), b"new".to_vec()),
                ("new_key".into(), b"v".to_vec()),
            ],
        )
        .unwrap();
    assert!(!result);
    // existing 应保持不变，new_key 也不应被设置
    assert_eq!(db.str_get(0, "new_key").unwrap(), None);
}
