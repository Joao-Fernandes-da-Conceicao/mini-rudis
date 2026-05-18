/// TCP 连接处理器（运行在 `spawn_local` 任务中）。
use std::rc::Rc;

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, trace};

use crate::cmd;
use crate::proto::{self, Frame};
use crate::script::ScriptEngine;
use crate::SharedDb;

const INITIAL_BUF_SIZE: usize = 4096;

pub async fn handle_connection(
    mut socket: TcpStream,
    db: SharedDb,
    script_engine: Rc<ScriptEngine>,
) -> std::io::Result<()> {
    let mut buf = BytesMut::with_capacity(INITIAL_BUF_SIZE);
    let mut db_index: usize = 0;

    loop {
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }

        loop {
            let frame = match proto::parse_frame(&mut buf) {
                Ok(Some(f)) => f,
                Ok(None) => break,
                Err(e) => {
                    let err_frame = Frame::error(e.to_string());
                    socket.write_all(&err_frame.serialize()).await?;
                    buf.clear();
                    break;
                }
            };

            trace!("recv frame: {:?}", frame);

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

            debug!(
                "cmd: {}",
                String::from_utf8_lossy(&args[0]).to_uppercase()
            );

            let response = match cmd::parse(&args) {
                Ok(command) => {
                    if matches!(command, cmd::Command::Quit) {
                        socket.write_all(&Frame::ok().serialize()).await?;
                        return Ok(());
                    }
                    cmd::execute(command, &db, &mut db_index, &script_engine)
                }
                Err(e) => Frame::from_error(&e),
            };

            trace!("resp: {:?}", response);

            socket.write_all(&response.serialize()).await?;
        }
    }
}
