/// ZSet（有序集合）数据类型集成测试
use mini_rudis::db::{Database, ScoreBound};

fn db() -> Database {
    Database::new(16)
}

fn zadd(db: &mut Database, key: &str, members: Vec<(f64, &str)>) {
    let members: Vec<(f64, String)> = members.into_iter().map(|(s, m)| (s, m.into())).collect();
    db.zadd(0, key, members, false, false, false, false, false)
        .unwrap();
}

#[test]
fn test_zadd_zscore() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b"), (3.0, "c")]);
    assert_eq!(db.zscore(0, "z", "a").unwrap(), Some(1.0));
    assert_eq!(db.zscore(0, "z", "d").unwrap(), None);
}

#[test]
fn test_zrank_zrevrank() {
    let mut db = db();
    zadd(&mut db, "z", vec![(10.0, "a"), (20.0, "b"), (30.0, "c")]);
    assert_eq!(db.zrank(0, "z", "a").unwrap(), Some(0));
    assert_eq!(db.zrank(0, "z", "c").unwrap(), Some(2));
    assert_eq!(db.zrevrank(0, "z", "c").unwrap(), Some(0));
    assert_eq!(db.zrevrank(0, "z", "a").unwrap(), Some(2));
    assert_eq!(db.zrank(0, "z", "missing").unwrap(), None);
}

#[test]
fn test_zcard() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b")]);
    assert_eq!(db.zcard(0, "z").unwrap(), 2);
    assert_eq!(db.zcard(0, "empty").unwrap(), 0);
}

#[test]
fn test_zrem() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b")]);
    let removed = db.zrem(0, "z", &["a".into()]).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(db.zscore(0, "z", "a").unwrap(), None);
    assert_eq!(db.zcard(0, "z").unwrap(), 1);
}

#[test]
fn test_zincrby() {
    let mut db = db();
    zadd(&mut db, "z", vec![(5.0, "m")]);
    let new_score = db.zincrby(0, "z", 3.5, "m").unwrap();
    assert!((new_score - 8.5).abs() < 1e-9);
    assert_eq!(db.zscore(0, "z", "m").unwrap(), Some(8.5));
}

#[test]
fn test_zrange_by_index() {
    let mut db = db();
    zadd(
        &mut db,
        "z",
        vec![(1.0, "a"), (2.0, "b"), (3.0, "c"), (4.0, "d")],
    );
    let result = db.zrange(0, "z", 0, 2, false, false).unwrap();
    let members: Vec<String> = result.into_iter().map(|(m, _)| m).collect();
    assert_eq!(members, vec!["a", "b", "c"]);
}

#[test]
fn test_zrange_rev() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b"), (3.0, "c")]);
    let result = db.zrange(0, "z", 0, -1, true, false).unwrap();
    let members: Vec<String> = result.into_iter().map(|(m, _)| m).collect();
    assert_eq!(members, vec!["c", "b", "a"]);
}

#[test]
fn test_zrangebyscore() {
    let mut db = db();
    zadd(
        &mut db,
        "z",
        vec![(1.0, "a"), (2.0, "b"), (3.0, "c"), (4.0, "d")],
    );
    let min = ScoreBound::inclusive(2.0);
    let max = ScoreBound::inclusive(3.0);
    let result = db.zrangebyscore(0, "z", min, max, false, 0, None).unwrap();
    let members: Vec<String> = result.into_iter().map(|(m, _)| m).collect();
    assert_eq!(members, vec!["b", "c"]);
}

#[test]
fn test_zrangebyscore_exclusive() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b"), (3.0, "c")]);
    let min = ScoreBound::exclusive(1.0);
    let max = ScoreBound::pos_inf();
    let result = db.zrangebyscore(0, "z", min, max, false, 0, None).unwrap();
    let members: Vec<String> = result.into_iter().map(|(m, _)| m).collect();
    assert_eq!(members, vec!["b", "c"]);
}

#[test]
fn test_zcount() {
    let mut db = db();
    zadd(
        &mut db,
        "z",
        vec![(1.0, "a"), (2.0, "b"), (3.0, "c"), (4.0, "d")],
    );
    let count = db
        .zcount(
            0,
            "z",
            ScoreBound::inclusive(2.0),
            ScoreBound::inclusive(3.0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_zpopmin_zpopmax() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b"), (3.0, "c")]);
    let min = db.zpopmin(0, "z", 1).unwrap();
    assert_eq!(min[0].0, "a");
    assert!((min[0].1 - 1.0).abs() < 1e-9);
    let max = db.zpopmax(0, "z", 1).unwrap();
    assert_eq!(max[0].0, "c");
}

#[test]
fn test_zremrangebyrank() {
    let mut db = db();
    zadd(
        &mut db,
        "z",
        vec![(1.0, "a"), (2.0, "b"), (3.0, "c"), (4.0, "d")],
    );
    let removed = db.zremrangebyrank(0, "z", 1, 2).unwrap();
    assert_eq!(removed, 2);
    assert_eq!(db.zcard(0, "z").unwrap(), 2);
}

#[test]
fn test_zremrangebyscore() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b"), (3.0, "c")]);
    let removed = db
        .zremrangebyscore(0, "z", ScoreBound::neg_inf(), ScoreBound::inclusive(2.0))
        .unwrap();
    assert_eq!(removed, 2);
    assert_eq!(db.zcard(0, "z").unwrap(), 1);
}

#[test]
fn test_zadd_nx_flag() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a")]);
    // NX 时不应该更新已存在的成员
    db.zadd(
        0,
        "z",
        vec![(100.0, "a".into())],
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(db.zscore(0, "z", "a").unwrap(), Some(1.0));
}

#[test]
fn test_zmscore() {
    let mut db = db();
    zadd(&mut db, "z", vec![(1.0, "a"), (2.0, "b")]);
    let scores = db
        .zmscore(0, "z", &["a".into(), "missing".into(), "b".into()])
        .unwrap();
    assert_eq!(scores[0], Some(1.0));
    assert_eq!(scores[1], None);
    assert_eq!(scores[2], Some(2.0));
}
