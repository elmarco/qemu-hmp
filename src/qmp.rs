// SPDX-License-Identifier: GPL-2.0-or-later

use std::io;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

use std::sync::Arc;

type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;

pub struct QmpConnection {
    writer: tokio::sync::Mutex<BufWriter<BoxWrite>>,
    response_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<serde_json::Value>>,
    event_rx: Mutex<mpsc::UnboundedReceiver<serde_json::Value>>,
    event_task: JoinHandle<()>,
    disconnected: Arc<Notify>,
    cpu_index: AtomicI64,
}

impl Drop for QmpConnection {
    fn drop(&mut self) {
        self.event_task.abort();
    }
}

impl QmpConnection {
    pub async fn connect(address: &str) -> io::Result<Self> {
        if let Some(addr) = address.strip_prefix("tcp:") {
            Self::connect_stream(TcpStream::connect(addr).await?).await
        } else {
            let path = address.strip_prefix("unix:").unwrap_or(address);
            Self::connect_stream(UnixStream::connect(path).await?).await
        }
    }

    async fn connect_stream(
        stream: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
    ) -> io::Result<Self> {
        let (read, write) = tokio::io::split(stream);
        let mut reader = BufReader::new(Box::new(read) as BoxRead);
        let mut writer = BufWriter::new(Box::new(write) as BoxWrite);

        // QMP negotiation: read greeting, send capabilities, read response.
        let mut greeting = String::new();
        reader.read_line(&mut greeting).await?;

        writer
            .write_all(b"{\"execute\":\"qmp_capabilities\"}\n")
            .await?;
        writer.flush().await?;

        let mut resp = String::new();
        reader.read_line(&mut resp).await?;
        let resp_val: serde_json::Value = serde_json::from_str(&resp)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if resp_val.get("error").is_some() {
            return Err(io::Error::other("QMP capabilities negotiation failed"));
        }

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();
        let disconnected = Arc::new(Notify::new());
        let disc = disconnected.clone();

        let handle = tokio::spawn(async move {
            let mut reader = reader;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&line) else {
                    continue;
                };
                if value.get("event").is_some() {
                    let _ = event_tx.send(value);
                } else {
                    let _ = response_tx.send(value);
                }
            }
            disc.notify_one();
        });

        Ok(Self {
            writer: tokio::sync::Mutex::new(writer),
            response_rx: tokio::sync::Mutex::new(response_rx),
            event_rx: Mutex::new(event_rx),
            event_task: handle,
            disconnected,
            cpu_index: AtomicI64::new(-1),
        })
    }

    pub async fn execute<C: qapi::Command>(&self, cmd: C) -> qapi::ExecuteResult<C> {
        #[derive(Serialize)]
        struct Envelope<'a, C: Serialize> {
            execute: &'a str,
            arguments: &'a C,
        }

        let envelope = Envelope {
            execute: C::NAME,
            arguments: &cmd,
        };
        let json = serde_json::to_value(&envelope).map_err(io::Error::from)?;
        let resp = self.execute_raw(&json).await?;

        if resp.get("error").is_some() {
            let err: qapi::Error = serde_json::from_value(resp).map_err(io::Error::from)?;
            Err(err.into())
        } else {
            let ret = resp
                .get("return")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            serde_json::from_value::<C::Ok>(ret).map_err(|e| io::Error::from(e).into())
        }
    }

    pub async fn execute_raw(&self, json: &serde_json::Value) -> io::Result<serde_json::Value> {
        let mut writer = self.writer.lock().await;
        let line = serde_json::to_string(json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        drop(writer);

        let mut rx = self.response_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "QMP connection closed"))
    }

    pub fn try_recv_event(&self) -> Option<serde_json::Value> {
        self.event_rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_recv()
            .ok()
    }

    pub fn is_disconnected(&self) -> bool {
        self.event_task.is_finished()
    }

    pub async fn wait_disconnected(&self) {
        self.disconnected.notified().await;
    }

    pub fn set_cpu_index(&self, index: i64) {
        self.cpu_index.store(index, Ordering::Relaxed);
    }

    pub fn cpu_index(&self) -> Option<i64> {
        match self.cpu_index.load(Ordering::Relaxed) {
            -1 => None,
            i => Some(i),
        }
    }
}
