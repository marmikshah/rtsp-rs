//! RTSP protocol implementation (RFC 2326).
//!
//! This module handles the text-based RTSP signaling protocol — parsing
//! requests, building responses, routing methods, and generating SDP.
//!
//! ## RTSP message format (RFC 2326 §4)
//!
//! RTSP messages follow HTTP/1.1 syntax with a different method set:
//!
//! ```text
//! DESCRIBE rtsp://server/stream RTSP/1.0\r\n
//! CSeq: 2\r\n
//! Accept: application/sdp\r\n
//! \r\n
//! ```
//!
//! Key differences from HTTP:
//! - Stateful: sessions persist across requests (RFC 2326 §3).
//! - Different methods: OPTIONS, DESCRIBE, SETUP, PLAY, PAUSE, TEARDOWN.
//! - Session header carries a server-assigned ID (RFC 2326 §12.37).
//!
//! ## Supported methods
//!
//! | Method | RFC section | Purpose |
//! |--------|-------------|---------|
//! | OPTIONS | §10.1 | Capability discovery |
//! | DESCRIBE | §10.2 | Retrieve SDP session description |
//! | SETUP | §10.4 | Negotiate transport (UDP ports) |
//! | PLAY | §10.5 | Start media delivery |
//! | PAUSE | §10.6 | Suspend media delivery |
//! | TEARDOWN | §10.7 | Destroy session |
//! | GET_PARAMETER | §10.8 | Keepalive / parameter query |

pub mod handler;
pub mod request;
pub mod response;
pub mod sdp;

pub use handler::MethodHandler;
pub use request::RtspRequest;
pub use response::RtspResponse;
