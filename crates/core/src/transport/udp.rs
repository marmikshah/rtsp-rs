use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use crate::error::Result;

/// UDP transport for outbound RTP packet delivery.
///
/// Binds a single ephemeral socket (`0.0.0.0:0`) and sends RTP packets
/// to client addresses resolved by the [`Server`](crate::Server).
///
/// This layer is deliberately address-only — it does not know about
/// sessions or mounts. The caller resolves session state to socket
/// addresses before calling [`send_to`](Self::send_to).
pub struct UdpTransport {
    socket: Arc<UdpSocket>,
}

impl UdpTransport {
    /// Bind an ephemeral UDP socket for outbound RTP.
    pub fn bind() -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            socket: Arc::new(socket),
        })
    }

    /// Send raw bytes to a specific socket address.
    pub fn send_to(&self, payload: &[u8], addr: SocketAddr) -> Result<usize> {
        Ok(self.socket.send_to(payload, addr)?)
    }
}
