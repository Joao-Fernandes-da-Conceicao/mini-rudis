//! 命令层集成测试共享辅助：`parse` + `execute`，与 `server` 使用相同的 `SharedDb` / `Rc<ScriptEngine>`。
#![allow(dead_code)]

use bytes::Bytes;
use mini_rudis::cmd::{execute, parse};
use mini_rudis::db::Database;
use mini_rudis::proto::Frame;
use mini_rudis::script::ScriptEngine;
use mini_rudis::SharedDb;
use std::cell::RefCell;
use std::rc::Rc;

pub fn shared_db() -> SharedDb {
    Rc::new(RefCell::new(Database::new(16)))
}

pub fn script_engine() -> Rc<ScriptEngine> {
    Rc::new(ScriptEngine::new())
}

pub fn argv(parts: &[&str]) -> Vec<Bytes> {
    parts
        .iter()
        .map(|s| Bytes::copy_from_slice(s.as_bytes()))
        .collect()
}

pub fn run_cmd(
    db: &SharedDb,
    se: &Rc<ScriptEngine>,
    idx: &mut usize,
    parts: &[&str],
) -> Frame {
    let cmd = parse(&argv(parts)).expect("parse");
    execute(cmd, db, idx, se)
}

pub fn bulk_frame(s: &str) -> Frame {
    Frame::Bulk(Bytes::copy_from_slice(s.as_bytes()))
}

pub fn int_frame(n: i64) -> Frame {
    Frame::Integer(n)
}

pub fn ok_frame() -> Frame {
    Frame::Simple("OK".into())
}

pub fn simple(s: &str) -> Frame {
    Frame::Simple(s.into())
}

pub fn assert_bulk_eq(f: &Frame, expected: &str) {
    match f {
        Frame::Bulk(b) => assert_eq!(std::str::from_utf8(b).unwrap(), expected),
        other => panic!("expected Bulk({expected}), got {other:?}"),
    }
}

pub fn assert_int_eq(f: &Frame, n: i64) {
    match f {
        Frame::Integer(v) => assert_eq!(*v, n),
        other => panic!("expected Integer({n}), got {other:?}"),
    }
}

pub fn assert_ok(f: &Frame) {
    assert_eq!(*f, ok_frame());
}

/// 数组中每项为 Bulk 字符串；`None` 表示 `$-1`（Null）
pub fn frame_array_bulks(f: Frame) -> Vec<Option<String>> {
    match f {
        Frame::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Frame::Bulk(b) => Some(String::from_utf8(b.to_vec()).unwrap()),
                Frame::Null => None,
                other => panic!("expected Bulk or Null, got {other:?}"),
            })
            .collect(),
        other => panic!("expected array: {other:?}"),
    }
}

pub fn frame_array_all_bulk(f: Frame) -> Vec<String> {
    match f {
        Frame::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Frame::Bulk(b) => String::from_utf8(b.to_vec()).unwrap(),
                other => panic!("expected Bulk, got {other:?}"),
            })
            .collect(),
        other => panic!("expected array: {other:?}"),
    }
}

pub fn sort_strings(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

pub fn bulk_float_approx(f: &Frame, expected: f64) {
    let s = match f {
        Frame::Bulk(b) => std::str::from_utf8(b).unwrap().parse::<f64>().unwrap(),
        other => panic!("expected Bulk float: {other:?}"),
    };
    assert!(
        (s - expected).abs() < 1e-5,
        "got {s}, expected ~{expected}"
    );
}
