//! Media codecs and RTP packetization.
//!
//! This module provides the [`Packetizer`] trait and codec-specific
//! implementations that convert raw encoded bitstreams into RTP packets.
//!
//! ## RTP overview (RFC 3550)
//!
//! Each encoded video frame is split into one or more RTP packets.
//! Every RTP packet carries a 12-byte fixed header ([`rtp::RtpHeader`])
//! containing:
//!
//! - **Sequence number** (16-bit, wrapping) — for reordering and loss detection.
//! - **Timestamp** (32-bit) — media clock, typically 90 kHz for video.
//! - **SSRC** (32-bit) — randomly chosen to identify the sender.
//! - **Marker bit** — set on the last packet of an access unit (frame).
//!
//! ## Supported codecs
//!
//! | Codec | Module | RFC | Status |
//! |-------|--------|-----|--------|
//! | H.264 | [`h264`] | [RFC 6184](https://tools.ietf.org/html/rfc6184) | Implemented |
//! | H.265 | [`h265`] | [RFC 7798](https://tools.ietf.org/html/rfc7798) | Planned |
//! | MJPEG | [`mjpeg`] | [RFC 2435](https://tools.ietf.org/html/rfc2435) | Planned |

pub mod h264;
pub mod h265;
pub mod mjpeg;
pub mod rtp;

/// Codec-specific RTP packetizer.
///
/// Each supported codec implements this trait, providing:
/// - **Packetization**: splitting encoded data into RTP-sized packets
/// - **SDP attributes**: codec parameters for the DESCRIBE response
/// - **RTP metadata**: payload type, clock rate, sequence/timestamp state
///
/// The generic RTP header is handled by [`rtp::RtpHeader`] — packetizers
/// compose it rather than reimplementing header serialization.
///
/// ## Implementing a new codec
///
/// 1. Create a new module (e.g. `media/aac.rs`)
/// 2. Implement `Packetizer` for your type
/// 3. Wire it into [`crate::mount::Mount`] via [`crate::Server::add_mount`]
pub trait Packetizer: Send {
    /// Packetize raw encoded data (e.g. Annex B bitstream) into RTP packets.
    ///
    /// Each returned `Vec<u8>` is a complete RTP packet: 12-byte header
    /// (RFC 3550 §5.1) followed by the codec-specific payload.
    ///
    /// `timestamp_increment` advances the RTP timestamp after this frame,
    /// typically `clock_rate / fps` (e.g. 3000 for 30 fps at 90 kHz).
    fn packetize(&mut self, encoded_data: &[u8], timestamp_increment: u32) -> Vec<Vec<u8>>;

    /// Codec name for the SDP `a=rtpmap` attribute (e.g. `"H264"`, `"H265"`).
    fn codec_name(&self) -> &'static str;

    /// RTP clock rate in Hz.
    ///
    /// Video codecs typically use 90000 (90 kHz) per RFC 3551 §4.
    fn clock_rate(&self) -> u32;

    /// RTP payload type number (RFC 3551).
    ///
    /// Dynamic types use 96–127. H.264 conventionally uses 96.
    fn payload_type(&self) -> u8;

    /// SDP media-level attribute lines for this codec.
    ///
    /// Returned strings include the `a=` prefix, e.g.:
    /// - `"a=rtpmap:96 H264/90000"`
    /// - `"a=fmtp:96 packetization-mode=1"`
    /// - `"a=control:track1"`
    fn sdp_attributes(&self) -> Vec<String>;

    /// Current RTP sequence number (for the `RTP-Info` header in PLAY responses).
    fn next_sequence(&self) -> u16;

    /// Current RTP timestamp as u32 (for the `RTP-Info` header in PLAY responses).
    fn next_rtp_timestamp(&self) -> u32;
}
