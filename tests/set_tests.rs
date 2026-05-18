/// Set 数据类型集成测试
use mini_rudis::db::Database;

fn db() -> Database {
    Database::new(16)
}

#[test]
fn test_sadd_smembers_scard() {
    let mut db = db();
    let added = db
        .sadd(0, "s", vec!["a".into(), "b".into(), "c".into()])
        .unwrap();
    assert_eq!(added, 3);
    assert_eq!(db.scard(0, "s").unwrap(), 3);
    let mut members = db.smembers(0, "s").unwrap();
    members.sort();
    assert_eq!(members, vec!["a", "b", "c"]);
}

#[test]
fn test_sadd_duplicate_ignored() {
    let mut db = db();
    db.sadd(0, "s", vec!["a".into()]).unwrap();
    let added = db.sadd(0, "s", vec!["a".into()]).unwrap();
    assert_eq!(added, 0);
    assert_eq!(db.scard(0, "s").unwrap(), 1);
}

#[test]
fn test_srem() {
    let mut db = db();
    db.sadd(0, "s", vec!["a".into(), "b".into()]).unwrap();
    let removed = db.srem(0, "s", &["a".into()]).unwrap();
    assert_eq!(removed, 1);
    assert!(!db.sismember(0, "s", "a").unwrap());
}

#[test]
fn test_sismember() {
    let mut db = db();
    db.sadd(0, "s", vec!["x".into()]).unwrap();
    assert!(db.sismember(0, "s", "x").unwrap());
    assert!(!db.sismember(0, "s", "y").unwrap());
}

#[test]
fn test_sunion() {
    let mut db = db();
    db.sadd(0, "s1", vec!["a".into(), "b".into()]).unwrap();
    db.sadd(0, "s2", vec!["b".into(), "c".into()]).unwrap();
    let mut union = db.sunion(0, &["s1".into(), "s2".into()]).unwrap();
    union.sort();
    assert_eq!(union, vec!["a", "b", "c"]);
}

#[test]
fn test_sinter() {
    let mut db = db();
    db.sadd(0, "s1", vec!["a".into(), "b".into(), "c".into()])
        .unwrap();
    db.sadd(0, "s2", vec!["b".into(), "c".into(), "d".into()])
        .unwrap();
    let mut inter = db.sinter(0, &["s1".into(), "s2".into()]).unwrap();
    inter.sort();
    assert_eq!(inter, vec!["b", "c"]);
}

#[test]
fn test_sdiff() {
    let mut db = db();
    db.sadd(0, "s1", vec!["a".into(), "b".into(), "c".into()])
        .unwrap();
    db.sadd(0, "s2", vec!["b".into(), "c".into()]).unwrap();
    let diff = db.sdiff(0, &["s1".into(), "s2".into()]).unwrap();
    assert_eq!(diff, vec!["a"]);
}

#[test]
fn test_sunionstore() {
    let mut db = db();
    db.sadd(0, "s1", vec!["a".into()]).unwrap();
    db.sadd(0, "s2", vec!["b".into()]).unwrap();
    let count = db
        .sunionstore(0, "dst", &["s1".into(), "s2".into()])
        .unwrap();
    assert_eq!(count, 2);
    let mut members = db.smembers(0, "dst").unwrap();
    members.sort();
    assert_eq!(members, vec!["a", "b"]);
}

#[test]
fn test_smove() {
    let mut db = db();
    db.sadd(0, "src", vec!["m".into()]).unwrap();
    db.sadd(0, "dst", vec!["x".into()]).unwrap();
    let result = db.smove(0, "src", "dst", "m").unwrap();
    assert!(result);
    assert!(!db.sismember(0, "src", "m").unwrap());
    assert!(db.sismember(0, "dst", "m").unwrap());
}

#[test]
fn test_smove_nonexistent_member() {
    let mut db = db();
    db.sadd(0, "src", vec!["a".into()]).unwrap();
    let result = db.smove(0, "src", "dst", "missing").unwrap();
    assert!(!result);
}

#[test]
fn test_smismember() {
    let mut db = db();
    db.sadd(0, "s", vec!["a".into(), "b".into()]).unwrap();
    let results = db
        .smismember(0, "s", &["a".into(), "b".into(), "c".into()])
        .unwrap();
    assert_eq!(results, vec![true, true, false]);
}
