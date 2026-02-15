//! MJPEG RTP packetizer — RFC 2435.
//!
//! Simpler than H.264/H.265:
//!
//! - Each JPEG frame maps to one or more RTP packets.
//! - RTP payload starts with an 8-byte JPEG-specific header
//!   (type, Q, width, height, fragment offset).
//! - No NAL unit concept — fragmentation is at the JPEG frame level.
//! - Uses static payload type 26: `a=rtpmap:26 JPEG/90000`
//!
//! ## Implementation plan
//!
//! Will implement [`super::Packetizer`] with JPEG frame splitting
//! and the RFC 2435 payload header. Good for IP cameras and
//! low-latency preview streams.
