//! # rtsp — RTSP server library for live media streaming
//!
//! A Rust library for publishing live media streams (H.264, with H.265 and
//! MJPEG planned) over the Real-Time Streaming Protocol (RTSP).
//!
//! ## Protocol references
//!
//! | RFC | Topic | How this crate uses it |
//! |-----|-------|----------------------|
//! | [RFC 2326](https://tools.ietf.org/html/rfc2326) | RTSP 1.0 | Request/response parsing, session lifecycle, transport negotiation |
//! | [RFC 3550](https://tools.ietf.org/html/rfc3550) | RTP | Packet header format, SSRC generation, sequence/timestamp semantics |
//! | [RFC 4566](https://tools.ietf.org/html/rfc4566) | SDP | Session description generation for DESCRIBE responses |
//! | [RFC 6184](https://tools.ietf.org/html/rfc6184) | H.264 RTP payload | NAL unit packetization, FU-A fragmentation, SDP fmtp attributes |
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │  Adapters (Python / GStreamer / CLI)      │
//! ├──────────────────────────────────────────┤
//! │  Server        — public API, orchestrator│
//! │  MountRegistry — named stream endpoints  │
//! ├──────────────────────────────────────────┤
//! │  Protocol      — RTSP parsing, SDP, etc. │
//! │  Session       — state machine, transport│
//! ├──────────────────────────────────────────┤
//! │  Transport     — TCP signaling, UDP data │
//! │  Media         — RTP header, packetizers │
//! └──────────────────────────────────────────┘
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use rtsp::Server;
//!
//! let mut server = Server::new("0.0.0.0:8554");
//! server.start().unwrap();
//!
//! // Push H.264 Annex B frames — the server packetizes and delivers via RTP.
//! // server.send_frame(&h264_data, 3000).unwrap();
//! ```
//!
//! ## Crate layout
//!
//! - [`server`] — High-level [`Server`] orchestrator and [`ServerConfig`].
//! - [`mount`] — [`Mount`] (stream endpoint) and [`MountRegistry`].
//! - [`protocol`] — RTSP request/response parsing, method handling, SDP generation.
//! - [`session`] — RTSP session state machine and transport negotiation.
//! - [`transport`] — TCP listener for RTSP signaling, UDP sender for RTP delivery.
//! - [`media`] — [`Packetizer`] trait, RTP header builder, codec implementations.
//! - [`error`] — [`RtspError`] enum and [`Result`] alias.

pub mod error;
pub mod media;
pub mod mount;
pub mod protocol;
pub mod server;
pub mod session;
pub mod transport;

pub use error::{Result, RtspError};
pub use media::Packetizer;
pub use mount::{DEFAULT_MOUNT_PATH, Mount, MountRegistry};
pub use server::{Server, ServerConfig, Viewer};
