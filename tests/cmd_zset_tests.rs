//! ZSet 命令层测试
mod common;

use common::*;
use mini_rudis::proto::Frame;

#[test]
fn cmd_zadd_zscore() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["ZADD", "z", "1", "a", "2", "b", "3", "c"],
        ),
        3,
    );
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["ZSCORE", "z", "a"]), "1");
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["ZSCORE", "z", "d"]),
        Frame::Null
    );
}

#[test]
fn cmd_zrank_zrevrank() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "z", "10", "a", "20", "b", "30", "c"],
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZRANK", "z", "a"]), 0);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZRANK", "z", "c"]), 2);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZREVRANK", "z", "c"]), 0);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZREVRANK", "z", "a"]), 2);
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["ZRANK", "z", "ghost"]),
        Frame::Null
    );
}

#[test]
fn cmd_zcard() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["ZADD", "z", "1", "a", "2", "b"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZCARD", "z"]), 2);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZCARD", "empty"]), 0);
}

#[test]
fn cmd_zrem() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["ZADD", "z", "1", "a", "2", "b"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZREM", "z", "a"]), 1);
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["ZSCORE", "z", "a"]),
        Frame::Null
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZCARD", "z"]), 1);
}

#[test]
fn cmd_zincrby() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["ZADD", "z", "5", "m"]);
    bulk_float_approx(
        &run_cmd(&db, &se, &mut idx, &["ZINCRBY", "z", "3.5", "m"]),
        8.5,
    );
}

#[test]
fn cmd_zrange_zrevrange() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "z", "1", "a", "2", "b", "3", "c", "4", "d"],
    );
    let m = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["ZRANGE", "z", "0", "2"]));
    assert_eq!(m, vec!["a", "b", "c"]);
    let rev = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZRANGE", "z", "0", "-1", "REV"],
    ));
    assert_eq!(rev, vec!["d", "c", "b", "a"]);
}

#[test]
fn cmd_zrangebyscore_zcount() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "z", "1", "a", "2", "b", "3", "c", "4", "d"],
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZCOUNT", "z", "2", "3"]), 2);
    let m = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZRANGEBYSCORE", "z", "2", "3"],
    ));
    assert_eq!(m, vec!["b", "c"]);
}

#[test]
fn cmd_zrangebyscore_exclusive_inf() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "z", "1", "a", "2", "b", "3", "c"],
    );
    let m = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZRANGEBYSCORE", "z", "(1", "+inf"],
    ));
    assert_eq!(m, vec!["b", "c"]);
}

#[test]
fn cmd_zpop() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "z", "1", "a", "2", "b", "3", "c"],
    );
    let f1 = run_cmd(&db, &se, &mut idx, &["ZPOPMIN", "z", "1"]);
    if let Frame::Array(p) = f1 {
        assert!(p.len() >= 2);
        assert_eq!(p[0], bulk_frame("a"));
    } else {
        panic!("zpopmin");
    }
    let f2 = run_cmd(&db, &se, &mut idx, &["ZPOPMAX", "z", "1"]);
    if let Frame::Array(p) = f2 {
        assert!(p.len() >= 2);
        assert_eq!(p[0], bulk_frame("c"));
    } else {
        panic!("zpopmax");
    }
}

#[test]
fn cmd_zremrangebyrank_score() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "z", "1", "a", "2", "b", "3", "c", "4", "d"],
    );
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["ZREMRANGEBYRANK", "z", "1", "2"]),
        2,
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["ZCARD", "z"]), 2);

    let db2 = shared_db();
    let mut idx2 = 0;
    run_cmd(
        &db2,
        &se,
        &mut idx2,
        &["ZADD", "z2", "1", "a", "2", "b", "3", "c"],
    );
    assert_int_eq(
        &run_cmd(
            &db2,
            &se,
            &mut idx2,
            &["ZREMRANGEBYSCORE", "z2", "-inf", "2"],
        ),
        2,
    );
    assert_int_eq(&run_cmd(&db2, &se, &mut idx2, &["ZCARD", "z2"]), 1);
}

#[test]
fn cmd_zadd_nx() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["ZADD", "z", "1", "a"]);
    run_cmd(&db, &se, &mut idx, &["ZADD", "z", "NX", "100", "a"]);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["ZSCORE", "z", "a"]), "1");
}

#[test]
fn cmd_zmscore() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["ZADD", "z", "1", "a", "2", "b"]);
    let v = frame_array_bulks(run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZMSCORE", "z", "a", "missing", "b"],
    ));
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].as_deref(), Some("1"));
    assert_eq!(v[1], None);
    assert_eq!(v[2].as_deref(), Some("2"));
}
