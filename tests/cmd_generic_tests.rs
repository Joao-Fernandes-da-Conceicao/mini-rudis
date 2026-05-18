//! 通用 Key 命令层测试（与 `generic_tests.rs` 对照）：`parse` + `execute`
mod common;

use common::*;
use mini_rudis::cmd::parse;
use mini_rudis::error::RedisError;
use mini_rudis::proto::Frame;
use std::time::Duration;

#[test]
fn cmd_del_single() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["DEL", "k"]), 1);
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["GET", "k"]),
        Frame::Null
    );
}

#[test]
fn cmd_del_multiple() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "a", "1"]);
    run_cmd(&db, &se, &mut idx, &["SET", "b", "2"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["DEL", "a", "b", "missing"]),
        2,
    );
}

#[test]
fn cmd_exists() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["EXISTS", "k"]), 1);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["EXISTS", "k", "k"]), 2);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["EXISTS", "missing"]), 0);
}

#[test]
fn cmd_type_of() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "str", "v"]);
    run_cmd(&db, &se, &mut idx, &["LPUSH", "list", "v"]);
    run_cmd(&db, &se, &mut idx, &["HSET", "hash", "f", "v"]);
    run_cmd(&db, &se, &mut idx, &["SADD", "set", "m"]);
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["ZADD", "zset", "1", "m"],
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["TYPE", "str"]),
        Frame::Simple("string".into())
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["TYPE", "list"]),
        Frame::Simple("list".into())
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["TYPE", "hash"]),
        Frame::Simple("hash".into())
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["TYPE", "set"]),
        Frame::Simple("set".into())
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["TYPE", "zset"]),
        Frame::Simple("zset".into())
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["TYPE", "none"]),
        Frame::Simple("none".into())
    );
}

#[test]
fn cmd_rename() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "old", "value"]);
    assert_ok(&run_cmd(&db, &se, &mut idx, &["RENAME", "old", "new"]));
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["GET", "old"]),
        Frame::Null
    );
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "new"]), "value");
}

#[test]
fn cmd_rename_missing_is_error() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    let f = run_cmd(&db, &se, &mut idx, &["RENAME", "missing", "new"]);
    assert!(matches!(f, Frame::Error(_)));
}

#[test]
fn cmd_renamenx() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "src", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["RENAMENX", "src", "dst"]), 1);
    run_cmd(&db, &se, &mut idx, &["SET", "src2", "v2"]);
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["RENAMENX", "src2", "dst"]),
        0,
    );
}

#[test]
fn cmd_expire_ttl_pttl() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["EXPIRE", "k", "100"]), 1);
    let ttl = match run_cmd(&db, &se, &mut idx, &["TTL", "k"]) {
        Frame::Integer(n) => n,
        other => panic!("ttl: {other:?}"),
    };
    assert!(ttl > 0 && ttl <= 100);
    let pttl = match run_cmd(&db, &se, &mut idx, &["PTTL", "k"]) {
        Frame::Integer(n) => n,
        other => panic!("pttl: {other:?}"),
    };
    assert!(pttl > 0 && pttl <= 100_000);
}

#[test]
fn cmd_ttl_no_expiry() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["TTL", "k"]), -1);
}

#[test]
fn cmd_ttl_missing() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["TTL", "missing"]), -2);
}

#[test]
fn cmd_persist() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "v"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["EXPIRE", "k", "100"]), 1);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["PERSIST", "k"]), 1);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["TTL", "k"]), -1);
}

#[test]
fn cmd_keys_pattern() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "user:1", "a"]);
    run_cmd(&db, &se, &mut idx, &["SET", "user:2", "b"]);
    run_cmd(&db, &se, &mut idx, &["SET", "post:1", "c"]);
    let mut k = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["KEYS", "user:*"],
    ));
    k.sort();
    assert_eq!(k, vec!["user:1", "user:2"]);
    let all = frame_array_all_bulk(run_cmd(&db, &se, &mut idx, &["KEYS", "*"]));
    assert_eq!(all.len(), 3);
}

#[test]
fn cmd_dbsize_flushdb() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "a", "1"]);
    run_cmd(&db, &se, &mut idx, &["SET", "b", "2"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["DBSIZE"]), 2);
    assert_ok(&run_cmd(&db, &se, &mut idx, &["FLUSHDB"]));
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["DBSIZE"]), 0);
}

#[test]
fn cmd_select_databases() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "k", "from_db0"]);
    assert_ok(&run_cmd(&db, &se, &mut idx, &["SELECT", "1"]));
    run_cmd(&db, &se, &mut idx, &["SET", "k", "from_db1"]);
    assert_ok(&run_cmd(&db, &se, &mut idx, &["SELECT", "0"]));
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "k"]), "from_db0");
    assert_ok(&run_cmd(&db, &se, &mut idx, &["SELECT", "1"]));
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["GET", "k"]), "from_db1");
}

#[test]
fn cmd_select_out_of_range_is_parse_error() {
    // `SELECT` 的库号在解析阶段校验；不会生成 `Command::Select`
    let r = parse(&argv(&["SELECT", "16"]));
    assert!(matches!(r, Err(RedisError::DbIndexOutOfRange)));
}

#[test]
fn cmd_key_expires_lazily() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_ok(&run_cmd(
        &db,
        &se,
        &mut idx,
        &["SET", "k", "v", "PX", "1"],
    ));
    std::thread::sleep(Duration::from_millis(10));
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["GET", "k"]),
        Frame::Null
    );
}

#[test]
fn cmd_msetnx_atomic() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["SET", "existing", "v"]);
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["MSETNX", "existing", "new", "new_key", "v"],
        ),
        0,
    );
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["GET", "new_key"]),
        Frame::Null
    );
}
