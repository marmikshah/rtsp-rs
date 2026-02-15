//! Network transport layer for RTSP signaling and RTP media delivery.
//!
//! RTSP uses a split transport model:
//!
//! - **TCP** ([`tcp`]): carries RTSP request/response signaling. One TCP
//!   connection per client, with a thread per connection.
//!
//! - **UDP** ([`udp`]): carries RTP media packets. A single ephemeral
//!   socket is shared for all outbound RTP delivery.
//!
//! Future: interleaved TCP transport (RFC 2326 §10.12) will multiplex
//! RTP data onto the RTSP TCP connection using `$` framing.

pub mod tcp;
pub mod udp;

pub use udp::UdpTransport;
