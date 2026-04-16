// SPDX-License-Identifier: GPL-2.0-or-later

use std::io;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use qapi::futures::QmpStreamTokio;
use qapi::qmp::Event;
use qapi::{Command, ExecuteResult};
use qapi_qmp::QmpCommand;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

/// Type-erased async I/O halves — the concrete type is hidden so
/// `QapiService` has a single monomorphisation for all transports.
type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;
type Service = qapi::futures::QapiService<QmpStreamTokio<BoxWrite>>;

/// An async connection to a QEMU instance over the QMP protocol.
///
/// Wraps a qapi-rs `QapiService` and a background event listener task.
/// Supports both Unix socket and TCP transports, type-erased at
/// construction time so that all callers see a single concrete type.
pub struct QmpConnection {
    service: Service,
    event_rx: Mutex<mpsc::UnboundedReceiver<Event>>,
    event_task: JoinHandle<()>,
    disconnected: Arc<Notify>,
    /// The default CPU index set by the `cpu` command.
    /// -1 means "not set" (use QEMU's default, i.e. first CPU).
    cpu_index: AtomicI64,
}

impl Drop for QmpConnection {
    fn drop(&mut self) {
        self.event_task.abort();
    }
}

impl QmpConnection {
    /// Connect to a QMP endpoint and perform the capability handshake.
    ///
    /// The address format determines the transport:
    /// - `tcp:<host>:<port>` -- connect via TCP
    /// - `unix:<path>` or just `<path>` -- connect via Unix domain socket
    pub async fn connect(address: &str) -> io::Result<Self> {
        if let Some(addr) = address.strip_prefix("tcp:") {
            Self::connect_stream(TcpStream::connect(addr).await?).await
        } else {
            let path = address.strip_prefix("unix:").unwrap_or(address);
            Self::connect_stream(UnixStream::connect(path).await?).await
        }
    }

    /// Split a connected stream into boxed halves, negotiate QMP
    /// capabilities, and spawn the event-loop task.
    async fn connect_stream(
        stream: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
    ) -> io::Result<Self> {
        let (read, write) = tokio::io::split(stream);
        let read: BoxRead = Box::new(read);
        let write: BoxWrite = Box::new(write);

        let stream = QmpStreamTokio::open_split(read, write).await?;
        let stream = stream.negotiate().await?;

        // Decompose into service + event stream instead of using
        // spawn_tokio(), so we can forward events to a channel.
        let (service, events) = stream.into_parts();
        events
            .release()
            .map_err(|()| io::Error::other("QMP events already abandoned"))?;

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let disconnected = Arc::new(Notify::new());
        let disc = disconnected.clone();

        let handle = tokio::spawn(async move {
            let mut events = events;
            while let Some(result) = events.next().await {
                match result {
                    Ok(event) => {
                        let _ = event_tx.send(event);
                    }
                    Err(_) => break,
                }
            }
            disc.notify_one();
        });

        Ok(Self {
            service,
            event_rx: Mutex::new(event_rx),
            event_task: handle,
            disconnected,
            cpu_index: AtomicI64::new(-1),
        })
    }

    /// Execute a typed QMP command and return the result.
    pub async fn execute<C: Command + QmpCommand>(&self, cmd: C) -> ExecuteResult<C> {
        self.service.execute(cmd).await
    }

    /// Try to receive a pending event without blocking.
    /// Returns `None` if no events are available.
    pub fn try_recv_event(&self) -> Option<Event> {
        self.event_rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_recv()
            .ok()
    }

    /// Check whether the QMP connection has been closed.
    ///
    /// Returns `true` once the background event-loop task has finished,
    /// which happens when QEMU closes the socket or the stream errors.
    pub fn is_disconnected(&self) -> bool {
        self.event_task.is_finished()
    }

    /// Wait until the QMP connection is closed.
    ///
    /// Resolves when QEMU closes the socket or the stream errors.
    pub async fn wait_disconnected(&self) {
        self.disconnected.notified().await;
    }

    /// Set the default CPU index (used by the `cpu` command).
    pub fn set_cpu_index(&self, index: i64) {
        self.cpu_index.store(index, Ordering::Relaxed);
    }

    /// Return the default CPU index, or `None` if not set.
    pub fn cpu_index(&self) -> Option<i64> {
        match self.cpu_index.load(Ordering::Relaxed) {
            -1 => None,
            i => Some(i),
        }
    }
}
