//! H.265 (HEVC) RTP packetizer — RFC 7798.
//!
//! Key differences from H.264 (RFC 6184):
//!
//! - **2-byte NAL unit header** (vs 1-byte in H.264).
//!   The NAL type is in bits 1..6 of the first byte.
//!
//! - **FU header format**: FU indicator (1 byte) + FU header (1 byte)
//!   with a 6-bit NAL type field.
//!
//! - **SDP attributes** (RFC 7798 §7.1):
//!   ```text
//!   a=rtpmap:96 H265/90000
//!   a=fmtp:96 sprop-vps=...; sprop-sps=...; sprop-pps=...
//!   ```
//!
//! ## Implementation plan
//!
//! Will follow the same pattern as [`super::h264::H264Packetizer`]:
//! - Compose an [`super::rtp::RtpHeader`] for generic header building.
//! - Implement [`super::Packetizer`] trait.
//! - Extract NAL units from Annex B (same start codes, different header parsing).
