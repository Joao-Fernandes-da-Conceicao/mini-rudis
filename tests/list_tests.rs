/// List 数据类型集成测试
use mini_rudis::db::Database;

fn db() -> Database {
    Database::new(16)
}

#[test]
fn test_lpush_rpush_llen() {
    let mut db = db();
    db.lpush(0, "l", vec![b"a".to_vec()]).unwrap();
    db.rpush(0, "l", vec![b"b".to_vec()]).unwrap();
    db.lpush(0, "l", vec![b"c".to_vec()]).unwrap();
    // 顺序：c, a, b
    assert_eq!(db.llen(0, "l").unwrap(), 3);
}

#[test]
fn test_lpop_single() {
    let mut db = db();
    db.rpush(0, "l", vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()])
        .unwrap();
    let v = db.lpop(0, "l", 1).unwrap();
    assert_eq!(v, vec![b"1".to_vec()]);
    assert_eq!(db.llen(0, "l").unwrap(), 2);
}

#[test]
fn test_rpop_multiple() {
    let mut db = db();
    db.rpush(0, "l", vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()])
        .unwrap();
    let v = db.rpop(0, "l", 2).unwrap();
    assert_eq!(v, vec![b"3".to_vec(), b"2".to_vec()]);
}

#[test]
fn test_lrange() {
    let mut db = db();
    db.rpush(
        0,
        "l",
        vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()],
    )
    .unwrap();
    let range = db.lrange(0, "l", 1, 2).unwrap();
    assert_eq!(range, vec![b"b".to_vec(), b"c".to_vec()]);
    let all = db.lrange(0, "l", 0, -1).unwrap();
    assert_eq!(all.len(), 4);
}

#[test]
fn test_lindex() {
    let mut db = db();
    db.rpush(0, "l", vec![b"x".to_vec(), b"y".to_vec(), b"z".to_vec()])
        .unwrap();
    assert_eq!(db.lindex(0, "l", 0).unwrap(), Some(b"x".to_vec()));
    assert_eq!(db.lindex(0, "l", -1).unwrap(), Some(b"z".to_vec()));
    assert_eq!(db.lindex(0, "l", 100).unwrap(), None);
}

#[test]
fn test_lset() {
    let mut db = db();
    db.rpush(0, "l", vec![b"a".to_vec(), b"b".to_vec()])
        .unwrap();
    db.lset(0, "l", 1, b"B".to_vec()).unwrap();
    assert_eq!(db.lindex(0, "l", 1).unwrap(), Some(b"B".to_vec()));
}

#[test]
fn test_linsert_before() {
    let mut db = db();
    db.rpush(0, "l", vec![b"a".to_vec(), b"c".to_vec()])
        .unwrap();
    db.linsert(0, "l", true, b"c", b"b".to_vec()).unwrap();
    let all = db.lrange(0, "l", 0, -1).unwrap();
    assert_eq!(all, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}

#[test]
fn test_lrem() {
    let mut db = db();
    db.rpush(
        0,
        "l",
        vec![
            b"a".to_vec(),
            b"b".to_vec(),
            b"a".to_vec(),
            b"c".to_vec(),
            b"a".to_vec(),
        ],
    )
    .unwrap();
    let removed = db.lrem(0, "l", 2, b"a").unwrap();
    assert_eq!(removed, 2);
    let all = db.lrange(0, "l", 0, -1).unwrap();
    assert_eq!(all, vec![b"b".to_vec(), b"c".to_vec(), b"a".to_vec()]);
}

#[test]
fn test_ltrim() {
    let mut db = db();
    db.rpush(
        0,
        "l",
        vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()],
    )
    .unwrap();
    db.ltrim(0, "l", 1, 2).unwrap();
    let all = db.lrange(0, "l", 0, -1).unwrap();
    assert_eq!(all, vec![b"b".to_vec(), b"c".to_vec()]);
}

#[test]
fn test_pop_empty_list_returns_empty() {
    let mut db = db();
    let result = db.lpop(0, "nonexistent", 1).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_lmove() {
    let mut db = db();
    db.rpush(0, "src", vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()])
        .unwrap();
    let moved = db.lmove(0, "src", "dst", false, true).unwrap();
    assert_eq!(moved, Some(b"3".to_vec()));
    assert_eq!(db.llen(0, "src").unwrap(), 2);
    assert_eq!(db.llen(0, "dst").unwrap(), 1);
}
