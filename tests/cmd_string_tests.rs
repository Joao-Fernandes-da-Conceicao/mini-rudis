//! String 命令：`parse` + `execute` 路径（与 `string_tests` 对照）
mod common;

use common::*;
use mini_rudis::proto::Frame;

#[test]
fn cmd_set_and_get() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_ok(&run_cmd(&db, &se, &mut idx, &["SET", "hello", "world"]));
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "hello"]), "world");
}

#[test]
fn cmd_get_nonexistent() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["GET", "missing"]),
        Frame::Null
    );
}

#[test]
fn cmd_overwrite() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v1"]);
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v2"]);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "k"]), "v2");
}

#[test]
fn cmd_setnx() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SETNX", "k", "v"]), 1);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["SETNX", "k", "v2"]), 0);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "k"]), "v");
}

#[test]
fn cmd_incr_from_zero() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["INCR", "counter"]), 1);
}

#[test]
fn cmd_incrby() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "n", "10"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["INCRBY", "n", "5"]), 15);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["INCRBY", "n", "-3"]), 12);
}

#[test]
fn cmd_incr_wrongtype() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "notanumber"]);
    let f = run_cmd(&db, &se, &mut idx, &["INCR", "k"]);
    assert!(matches!(f, Frame::Error(_)));
}

#[test]
fn cmd_append_strlen() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "hello"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["APPEND", "k", " world"]), 11);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["STRLEN", "k"]), 11);
}

#[test]
fn cmd_mset_mget() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_ok(&run_cmd(
        &db,
        &se,
        &mut idx,
        &["MSET", "a", "1", "b", "2", "c", "3"],
    ));
    let f = run_cmd(&db, &se, &mut idx, &["MGET", "a", "b", "missing"]);
    let vals = frame_array_bulks(f);
    assert_eq!(vals.len(), 3);
    assert_eq!(vals[0].as_deref(), Some("1"));
    assert_eq!(vals[1].as_deref(), Some("2"));
    assert_eq!(vals[2], None);
}

#[test]
fn cmd_getset() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "old"]);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GETSET", "k", "new"]), "old");
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "k"]), "new");
}

#[test]
fn cmd_getdel() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v"]);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GETDEL", "k"]), "v");
    assert_eq!(run_cmd(&db, &se, &mut idx, &["GET", "k"]), Frame::Null);
}

#[test]
fn cmd_getrange() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "Hello World"]);
    assert_bulk_eq(
        &run_cmd(&db, &se, &mut idx, &["GETRANGE", "k", "0", "4"]),
        "Hello",
    );
    assert_bulk_eq(
        &run_cmd(&db, &se, &mut idx, &["GETRANGE", "k", "-5", "-1"]),
        "World",
    );
}

#[test]
fn cmd_setrange() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "Hello World"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["SETRANGE", "k", "6", "Redis"]),
        11,
    );
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "k"]), "Hello Redis");
}

#[test]
fn cmd_incrbyfloat() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "f", "10.5"]);
    bulk_float_approx(
        &run_cmd(&db, &se, &mut idx, &["INCRBYFLOAT", "f", "0.1"]),
        10.6,
    );
}

#[test]
fn cmd_decr_decrby() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "n", "10"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["DECR", "n"]), 9);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["DECRBY", "n", "4"]), 5);
}

#[test]
fn cmd_setex_psetex() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_ok(&run_cmd(&db, &se, &mut idx, &["SETEX", "a", "3600", "x"]));
    assert_ok(&run_cmd(
        &db,
        &se,
        &mut idx,
        &["PSETEX", "b", "3600000", "y"],
    ));
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "a"]), "x");
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "b"]), "y");
}

#[test]
fn cmd_msetnx() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "existing", "v"]);
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["MSETNX", "existing", "new", "new_key", "v2"],
        ),
        0,
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["GET", "new_key"]),
        Frame::Null
    );
}

#[test]
fn cmd_wrongtype_get_on_list() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["LPUSH", "list", "v"]);
    let f = run_cmd(&db, &se, &mut idx, &["GET", "list"]);
    assert!(matches!(f, Frame::Error(_)));
}
