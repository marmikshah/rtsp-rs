use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::mount::MountRegistry;
use crate::protocol::MethodHandler;
use crate::protocol::RtspRequest;
use crate::server::ServerConfig;
use crate::session::SessionManager;

/// Non-blocking TCP accept loop.
///
/// Checks the `running` flag between accepts with a 50ms poll interval
/// so that [`crate::server::Server::stop`] can terminate it promptly.
pub fn accept_loop(
    listener: TcpListener,
    session_manager: SessionManager,
    mounts: MountRegistry,
    config: Arc<ServerConfig>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let sm = session_manager.clone();
                let r = running.clone();
                let m = mounts.clone();
                let c = config.clone();
                thread::spawn(move || {
                    Connection::handle(stream, sm, m, c, r);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                if running.load(Ordering::SeqCst) {
                    tracing::warn!(error = %e, "TCP accept error");
                }
            }
        }
    }
    tracing::debug!("accept loop exited");
}

/// A single RTSP client connection with its own lifecycle.
struct Connection {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    handler: MethodHandler,
    peer_addr: SocketAddr,
}

impl Connection {
    /// Entry point: set up a connection and run its request loop.
    pub fn handle(
        stream: TcpStream,
        session_manager: SessionManager,
        mounts: MountRegistry,
        config: Arc<ServerConfig>,
        running: Arc<AtomicBool>,
    ) {
        let peer_addr = match stream.peer_addr() {
            Ok(addr) => addr,
            Err(_) => return,
        };

        tracing::info!(%peer_addr, "client connected");

        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };

        let handler =
            MethodHandler::new(session_manager.clone(), peer_addr, mounts.clone(), config);

        let mut conn = Connection {
            reader: BufReader::new(reader_stream),
            writer: stream,
            handler,
            peer_addr,
        };

        let reason = conn.run(&running);
        conn.cleanup(&session_manager, &mounts);

        tracing::info!(%peer_addr, reason, "client disconnected");
    }

    /// RTSP request/response loop. Returns the reason for exiting.
    fn run(&mut self, running: &Arc<AtomicBool>) -> &'static str {
        while running.load(Ordering::SeqCst) {
            let mut request_text = String::new();
            loop {
                let mut line = String::new();
                match self.reader.read_line(&mut line) {
                    Ok(0) => return "connection closed by client",
                    Ok(_) => {
                        request_text.push_str(&line);
                        if line == "\r\n" || line == "\n" {
                            break;
                        }
                    }
                    Err(_) => return "read error",
                }
            }

            if request_text.trim().is_empty() {
                continue;
            }

            match RtspRequest::parse(&request_text) {
                Ok(request) => {
                    tracing::debug!(
                        peer = %self.peer_addr,
                        method = %request.method,
                        uri = %request.uri,
                        version = %request.version,
                        "request"
                    );

                    let response = self.handler.handle(&request);

                    tracing::debug!(
                        peer = %self.peer_addr,
                        status = response.status_code,
                        "response"
                    );

                    if self
                        .writer
                        .write_all(response.serialize().as_bytes())
                        .is_err()
                    {
                        return "write error";
                    }
                }
                Err(e) => {
                    tracing::warn!(peer = %self.peer_addr, error = %e, "parse error");
                }
            }
        }

        "server shutting down"
    }

    /// Clean up sessions owned by this connection and unsubscribe from mounts.
    fn cleanup(&self, session_manager: &SessionManager, mounts: &MountRegistry) {
        let orphaned = self.handler.session_ids().to_vec();
        if !orphaned.is_empty() {
            for id in &orphaned {
                mounts.unsubscribe_all(id);
            }
            let removed = session_manager.remove_sessions(&orphaned);
            tracing::info!(peer = %self.peer_addr, removed, "cleaned up sessions on disconnect");
        }
    }
}
