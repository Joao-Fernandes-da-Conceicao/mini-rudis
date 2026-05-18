/// TCP 连接处理器
///
/// 每个客户端连接由一个独立的 Tokio 任务处理。
/// 流程：读取 RESP 帧 → 解析命令 → 执行 → 写回响应
use std::sync::Arc;

use bytes::BytesMut;
use parking_lot::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, trace};

use crate::cmd;
use crate::db::Database;
use crate::proto::{self, Frame};
use crate::script::ScriptEngine;

/// 读缓冲区初始大小（字节）
const INITIAL_BUF_SIZE: usize = 4096;

/// 处理单个客户端连接
pub async fn handle_connection(
    mut socket: TcpStream,
    db: Arc<Mutex<Database>>,
    script_engine: Arc<ScriptEngine>,
) -> std::io::Result<()> {
    let mut buf = BytesMut::with_capacity(INITIAL_BUF_SIZE);
    // 每个连接维护独立的当前数据库索引（默认 db 0）
    let mut db_index: usize = 0;

    loop {
        // ── 读取数据到缓冲区 ──────────────────────────────────────────
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            // 客户端关闭连接
            return Ok(());
        }

        // ── 从缓冲区解析 RESP 帧（可能有多个粘连的命令）────────────────
        loop {
            let frame = match proto::parse_frame(&mut buf) {
                Ok(Some(f)) => f,
                Ok(None) => break, // 数据不完整，等待更多数据
                Err(e) => {
                    let err_frame = Frame::error(e.to_string());
                    socket.write_all(&err_frame.serialize()).await?;
                    // 协议错误后清空缓冲区，等待客户端重新发送
                    buf.clear();
                    break;
                }
            };

            trace!("recv frame: {:?}", frame);

            // ── 将帧解析为命令参数列表 ──────────────────────────────────
            let args = match proto::frame_to_cmd_args(frame) {
                Ok(a) => a,
                Err(e) => {
                    let err_frame = Frame::error(e.to_string());
                    socket.write_all(&err_frame.serialize()).await?;
                    continue;
                }
            };

            if args.is_empty() {
                continue;
            }

            debug!("cmd: {}", String::from_utf8_lossy(&args[0]).to_uppercase());

            // ── 解析并执行命令 ──────────────────────────────────────────
            let response = match cmd::parse(&args) {
                Ok(command) => {
                    // QUIT 命令特殊处理：发送 OK 后关闭连接
                    if matches!(command, cmd::Command::Quit) {
                        socket.write_all(&Frame::ok().serialize()).await?;
                        return Ok(());
                    }
                    cmd::execute(command, &db, &mut db_index, &script_engine)
                }
                Err(e) => Frame::from_error(&e),
            };

            trace!("resp: {:?}", response);

            // ── 将响应帧写回客户端 ──────────────────────────────────────
            socket.write_all(&response.serialize()).await?;
        }
    }
}
