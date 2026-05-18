//! Hash 命令层测试
mod common;

use common::*;
use mini_rudis::proto::Frame;

#[test]
fn cmd_hset_hget() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["HSET", "h", "name", "Alice"],
        ),
        1,
    );
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["HGET", "h", "name"]), "Alice");
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["HGET", "h", "missing"]),
        Frame::Null
    );
}

#[test]
fn cmd_hset_multi() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["HSET", "h", "f1", "v1", "f2", "v2"],
        ),
        2,
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["HLEN", "h"]), 2);
}

#[test]
fn cmd_hsetnx() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "f", "old"]);
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["HSETNX", "h", "f", "new"],
        ),
        0,
    );
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["HGET", "h", "f"]), "old");
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["HSETNX", "h", "new_field", "val"],
        ),
        1,
    );
}

#[test]
fn cmd_hdel() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "f1", "v1", "f2", "v2"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["HDEL", "h", "f1"]), 1);
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["HGET", "h", "f1"]),
        Frame::Null
    );
}

#[test]
fn cmd_hexists() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "f", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["HEXISTS", "h", "f"]), 1);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["HEXISTS", "h", "missing"]),
        0,
    );
}

#[test]
fn cmd_hgetall_len() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "a", "1", "b", "2"]);
    let f = run_cmd(&db, &se, &mut idx, &["HGETALL", "h"]);
    if let Frame::Array(parts) = f {
        assert_eq!(parts.len(), 4);
    } else {
        panic!("not array");
    }
}

#[test]
fn cmd_hkeys_sorted() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "x", "10", "y", "20"]);
    let mut keys = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["HKEYS", "h"]));
    keys.sort();
    assert_eq!(keys, vec!["x", "y"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["HLEN", "h"]), 2);
}

#[test]
fn cmd_hincrby() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "views", "100"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["HINCRBY", "h", "views", "50"]),
        150,
    );
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["HINCRBY", "h", "views", "-10"]),
        140,
    );
}

#[test]
fn cmd_hincrbyfloat() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "price", "10.5"]);
    bulk_float_approx(
        &run_cmd(&db, &se, &mut idx, &["HINCRBYFLOAT", "h", "price", "1.1"]),
        11.6,
    );
}

#[test]
fn cmd_hmget() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["HSET", "h", "a", "1"]);
    let v = frame_array_bulks(run_cmd(
        &db,
        &se,
        &mut idx,
        &["HMGET", "h", "a", "b"],
    ));
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].as_deref(), Some("1"));
    assert_eq!(v[1], None);
}

#[test]
fn cmd_hmset_compat() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_ok(&run_cmd(
        &db,
        &se,
        &mut idx,
        &["HMSET", "h", "f1", "v1", "f2", "v2"],
    ));
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["HGET", "h", "f2"]), "v2");
}
