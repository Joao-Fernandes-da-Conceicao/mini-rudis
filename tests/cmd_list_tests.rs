//! List 命令层测试（与 `list_tests` 对照）
mod common;

use common::*;
use mini_rudis::proto::Frame;

#[test]
fn cmd_lpush_rpush_llen() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["LPUSH", "l", "a"]);
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "b"]);
    run_cmd(&db, &se, &mut idx, &["LPUSH", "l", "c"]);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["LLEN", "l"]), 3);
}

#[test]
fn cmd_lpop_single() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "1", "2", "3"]);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["LPOP", "l"]), "1");
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["LLEN", "l"]), 2);
}

#[test]
fn cmd_rpop_count() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "1", "2", "3"]);
    let f = run_cmd(&db, &se, &mut idx, &["RPOP", "l", "2"]);
    let parts = frame_array_all_bulk(f);
    assert_eq!(parts, vec!["3".to_string(), "2".to_string()]);
}

#[test]
fn cmd_lrange() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "a", "b", "c", "d"]);
    let r = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["LRANGE", "l", "1", "2"],
    ));
    assert_eq!(r, vec!["b", "c"]);
    let all = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["LRANGE", "l", "0", "-1"],
    ));
    assert_eq!(all.len(), 4);
}

#[test]
fn cmd_lindex() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "x", "y", "z"]);
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["LINDEX", "l", "0"]), "x");
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["LINDEX", "l", "-1"]), "z");
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["LINDEX", "l", "100"]),
        Frame::Null
    );
}

#[test]
fn cmd_lset() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "a", "b"]);
    assert_ok(&run_cmd(
        &db,
        &se,
        &mut idx,
        &["LSET", "l", "1", "B"],
    ));
    assert_bulk_eq(&run_cmd(&db, &se, &mut idx, &["LINDEX", "l", "1"]), "B");
}

#[test]
fn cmd_linsert() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "l", "a", "c"]);
    assert_int_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["LINSERT", "l", "BEFORE", "c", "b"],
        ),
        3,
    );
    let all = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["LRANGE", "l", "0", "-1"],
    ));
    assert_eq!(all, vec!["a", "b", "c"]);
}

#[test]
fn cmd_lrem() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["RPUSH", "l", "a", "b", "a", "c", "a"],
    );
    assert_int_eq(
        &run_cmd(&db, &se, &mut idx, &["LREM", "l", "2", "a"]),
        2,
    );
    let all = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["LRANGE", "l", "0", "-1"],
    ));
    assert_eq!(all, vec!["b", "c", "a"]);
}

#[test]
fn cmd_ltrim() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(
        &db,
        &se,
        &mut idx,
        &["RPUSH", "l", "a", "b", "c", "d"],
    );
    assert_ok(&run_cmd(&db, &se, &mut idx, &["LTRIM", "l", "1", "2"]));
    let all = frame_array_all_bulk(run_cmd(
        &db,
        &se,
        &mut idx,
        &["LRANGE", "l", "0", "-1"],
    ));
    assert_eq!(all, vec!["b", "c"]);
}

#[test]
fn cmd_lpop_empty() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    assert_eq!(
        run_cmd(&db, &se, &mut idx, &["LPOP", "ghost", "1"]),
        Frame::Null
    );
}

#[test]
fn cmd_lmove() {
    let db = shared_db();
    let se = script_engine();
    let mut idx = 0;
    run_cmd(&db, &se, &mut idx, &["RPUSH", "src", "1", "2", "3"]);
    assert_bulk_eq(
        &run_cmd(
            &db,
            &se,
            &mut idx,
            &["LMOVE", "src", "dst", "RIGHT", "LEFT"],
        ),
        "3",
    );
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["LLEN", "src"]), 2);
    assert_int_eq(&run_cmd(&db, &se, &mut idx, &["LLEN", "dst"]), 1);
}
