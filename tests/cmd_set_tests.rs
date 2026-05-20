//! Set 命令层测试
mod common;

use common::*;
use mini_rudis::proto::Frame;

#[test]
fn cmd_sadd_smembers_scard() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["SADD", "s", "a", "b", "c"]),
        3,
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SCARD", "s"]), 3);
    let mut m = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["SMEMBERS", "s"]));
    m.sort();
    assert_eq!(m, vec!["a", "b", "c"]);
}

#[test]
fn cmd_sadd_dup() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "s", "a"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SADD", "s", "a"]), 0);
}

#[test]
fn cmd_srem_sismember() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "s", "a", "b"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SREM", "s", "a"]), 1);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SISMEMBER", "s", "a"]), 0);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SISMEMBER", "s", "b"]), 1);
}

#[test]
fn cmd_sunion_sinter_sdiff() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "s1", "a", "b"]);
    run_cmd(&db, &se, &mut idx, &["SADD", "s2", "b", "c"]);
    let mut u = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["SUNION", "s1", "s2"]));
    u.sort();
    assert_eq!(u, vec!["a", "b", "c"]);
    let mut i = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["SINTER", "s1", "s2"]));
    i.sort();
    assert_eq!(i, vec!["b"]);
    let d = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["SDIFF", "s1", "s2"]));
    assert_eq!(d, vec!["a"]);
}

#[test]
fn cmd_sunionstore() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "s1", "a"]);
    run_cmd(&db, &se, &mut idx, &["SADD", "s2", "b"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["SUNIONSTORE", "dst", "s1", "s2"]),
        2,
    );
    let mut m = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["SMEMBERS", "dst"]));
    m.sort();
    assert_eq!(m, vec!["a", "b"]);
}

#[test]
fn cmd_smove() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "src", "m"]);
    run_cmd(&db, &se, &mut idx, &["SADD", "dst", "x"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["SMOVE", "src", "dst", "m"]),
        1,
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SISMEMBER", "src", "m"]), 0);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SISMEMBER", "dst", "m"]), 1);
}

#[test]
fn cmd_smove_miss() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "src", "a"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["SMOVE", "src", "dst", "missing"]),
        0,
    );
}

#[test]
fn cmd_smismember() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SADD", "s", "a", "b"]);
    let f = run_cmd(&db, &se, &mut idx, &["SMISMEMBER", "s", "a", "b", "c"]);
    if let Frame::Array(xs) = f {
        assert_eq!(xs.len(), 3);
        assert_eq!(xs[0], Frame::Integer(1));
        assert_eq!(xs[1], Frame::Integer(1));
        assert_eq!(xs[2], Frame::Integer(0));
    } else {
        panic!("not array");
    }
}
