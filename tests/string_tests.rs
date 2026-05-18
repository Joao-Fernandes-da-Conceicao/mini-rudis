/// String 数据类型集成测试
use mini_rudis::db::Database;

fn db() -> Database {
    Database::new(16)
}

#[test]
fn test_set_and_get() {
    let mut db = db();
    db.str_set(0, "hello".into(), b"world".to_vec(), None)
        .unwrap();
    assert_eq!(db.str_get(0, "hello").unwrap(), Some(b"world".to_vec()));
}

#[test]
fn test_get_nonexistent_key() {
    let db = db();
    assert_eq!(db.str_get(0, "missing").unwrap(), None);
}

#[test]
fn test_overwrite_value() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v1".to_vec(), None).unwrap();
    db.str_set(0, "k".into(), b"v2".to_vec(), None).unwrap();
    assert_eq!(db.str_get(0, "k").unwrap(), Some(b"v2".to_vec()));
}

#[test]
fn test_setnx_success() {
    let mut db = db();
    assert!(db.str_setnx(0, "k".into(), b"v".to_vec()).unwrap());
    assert_eq!(db.str_get(0, "k").unwrap(), Some(b"v".to_vec()));
}

#[test]
fn test_setnx_fail_if_exists() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v1".to_vec(), None).unwrap();
    assert!(!db.str_setnx(0, "k".into(), b"v2".to_vec()).unwrap());
    assert_eq!(db.str_get(0, "k").unwrap(), Some(b"v1".to_vec()));
}

#[test]
fn test_incr_from_zero() {
    let mut db = db();
    let result = db.str_incr(0, "counter", 1).unwrap();
    assert_eq!(result, 1);
}

#[test]
fn test_incrby() {
    let mut db = db();
    db.str_set(0, "n".into(), b"10".to_vec(), None).unwrap();
    assert_eq!(db.str_incr(0, "n", 5).unwrap(), 15);
    assert_eq!(db.str_incr(0, "n", -3).unwrap(), 12);
}

#[test]
fn test_incr_non_integer_error() {
    let mut db = db();
    db.str_set(0, "k".into(), b"notanumber".to_vec(), None)
        .unwrap();
    assert!(db.str_incr(0, "k", 1).is_err());
}

#[test]
fn test_append_and_strlen() {
    let mut db = db();
    db.str_set(0, "k".into(), b"hello".to_vec(), None).unwrap();
    let new_len = db.str_append(0, "k", b" world").unwrap();
    assert_eq!(new_len, 11);
    assert_eq!(db.str_strlen(0, "k").unwrap(), 11);
}

#[test]
fn test_mset_mget() {
    let mut db = db();
    db.mset(
        0,
        vec![
            ("a".into(), b"1".to_vec()),
            ("b".into(), b"2".to_vec()),
            ("c".into(), b"3".to_vec()),
        ],
    )
    .unwrap();
    let vals = db
        .mget(0, &["a".into(), "b".into(), "missing".into()])
        .unwrap();
    assert_eq!(vals[0], Some(b"1".to_vec()));
    assert_eq!(vals[1], Some(b"2".to_vec()));
    assert_eq!(vals[2], None);
}

#[test]
fn test_getset() {
    let mut db = db();
    db.str_set(0, "k".into(), b"old".to_vec(), None).unwrap();
    let old = db.str_getset(0, "k".into(), b"new".to_vec()).unwrap();
    assert_eq!(old, Some(b"old".to_vec()));
    assert_eq!(db.str_get(0, "k").unwrap(), Some(b"new".to_vec()));
}

#[test]
fn test_getdel() {
    let mut db = db();
    db.str_set(0, "k".into(), b"v".to_vec(), None).unwrap();
    let val = db.str_getdel(0, "k").unwrap();
    assert_eq!(val, Some(b"v".to_vec()));
    assert_eq!(db.str_get(0, "k").unwrap(), None);
}

#[test]
fn test_getrange() {
    let mut db = db();
    db.str_set(0, "k".into(), b"Hello World".to_vec(), None)
        .unwrap();
    let sub = db.str_getrange(0, "k", 0, 4).unwrap();
    assert_eq!(sub, b"Hello");
    let sub2 = db.str_getrange(0, "k", -5, -1).unwrap();
    assert_eq!(sub2, b"World");
}

#[test]
fn test_setrange() {
    let mut db = db();
    db.str_set(0, "k".into(), b"Hello World".to_vec(), None)
        .unwrap();
    db.str_setrange(0, "k", 6, b"Redis").unwrap();
    assert_eq!(db.str_get(0, "k").unwrap(), Some(b"Hello Redis".to_vec()));
}

#[test]
fn test_incr_float() {
    let mut db = db();
    db.str_set(0, "f".into(), b"10.5".to_vec(), None).unwrap();
    let result = db.str_incr_float(0, "f", 0.1).unwrap();
    assert!((result - 10.6).abs() < 1e-9);
}

#[test]
fn test_wrongtype_error() {
    let mut db = db();
    db.lpush(0, "list", vec![b"v".to_vec()]).unwrap();
    assert!(db.str_get(0, "list").is_err());
}
