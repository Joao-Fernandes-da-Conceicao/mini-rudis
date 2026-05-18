/// Hash 数据类型集成测试
use mini_rudis::db::Database;

fn db() -> Database {
    Database::new(16)
}

#[test]
fn test_hset_hget() {
    let mut db = db();
    db.hset(0, "h", vec![("name".into(), b"Alice".to_vec())])
        .unwrap();
    assert_eq!(db.hget(0, "h", "name").unwrap(), Some(b"Alice".to_vec()));
    assert_eq!(db.hget(0, "h", "missing").unwrap(), None);
}

#[test]
fn test_hset_multiple_fields() {
    let mut db = db();
    let new = db
        .hset(
            0,
            "h",
            vec![("f1".into(), b"v1".to_vec()), ("f2".into(), b"v2".to_vec())],
        )
        .unwrap();
    assert_eq!(new, 2);
    assert_eq!(db.hlen(0, "h").unwrap(), 2);
}

#[test]
fn test_hsetnx() {
    let mut db = db();
    db.hset(0, "h", vec![("f".into(), b"old".to_vec())])
        .unwrap();
    assert!(!db.hsetnx(0, "h", "f", b"new".to_vec()).unwrap());
    assert_eq!(db.hget(0, "h", "f").unwrap(), Some(b"old".to_vec()));
    assert!(db.hsetnx(0, "h", "new_field", b"val".to_vec()).unwrap());
}

#[test]
fn test_hdel() {
    let mut db = db();
    db.hset(
        0,
        "h",
        vec![("f1".into(), b"v1".to_vec()), ("f2".into(), b"v2".to_vec())],
    )
    .unwrap();
    let deleted = db.hdel(0, "h", &["f1".into()]).unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(db.hget(0, "h", "f1").unwrap(), None);
    assert_eq!(db.hget(0, "h", "f2").unwrap(), Some(b"v2".to_vec()));
}

#[test]
fn test_hexists() {
    let mut db = db();
    db.hset(0, "h", vec![("f".into(), b"v".to_vec())]).unwrap();
    assert!(db.hexists(0, "h", "f").unwrap());
    assert!(!db.hexists(0, "h", "missing").unwrap());
}

#[test]
fn test_hgetall() {
    let mut db = db();
    db.hset(
        0,
        "h",
        vec![("a".into(), b"1".to_vec()), ("b".into(), b"2".to_vec())],
    )
    .unwrap();
    let pairs = db.hgetall(0, "h").unwrap();
    assert_eq!(pairs.len(), 2);
}

#[test]
fn test_hkeys_hvals() {
    let mut db = db();
    db.hset(
        0,
        "h",
        vec![("x".into(), b"10".to_vec()), ("y".into(), b"20".to_vec())],
    )
    .unwrap();
    let mut keys = db.hkeys(0, "h").unwrap();
    keys.sort();
    assert_eq!(keys, vec!["x", "y"]);
    assert_eq!(db.hvals(0, "h").unwrap().len(), 2);
}

#[test]
fn test_hincrby() {
    let mut db = db();
    db.hset(0, "h", vec![("views".into(), b"100".to_vec())])
        .unwrap();
    assert_eq!(db.hincrby(0, "h", "views", 50).unwrap(), 150);
    assert_eq!(db.hincrby(0, "h", "views", -10).unwrap(), 140);
}

#[test]
fn test_hincrbyfloat() {
    let mut db = db();
    db.hset(0, "h", vec![("price".into(), b"10.5".to_vec())])
        .unwrap();
    let result = db.hincrbyfloat(0, "h", "price", 1.1).unwrap();
    assert!((result - 11.6).abs() < 1e-9);
}

#[test]
fn test_hmget_with_missing() {
    let mut db = db();
    db.hset(0, "h", vec![("a".into(), b"1".to_vec())]).unwrap();
    let vals = db.hmget(0, "h", &["a".into(), "b".into()]).unwrap();
    assert_eq!(vals[0], Some(b"1".to_vec()));
    assert_eq!(vals[1], None);
}
