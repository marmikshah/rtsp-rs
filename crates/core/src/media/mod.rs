pub mod rtp;
pub mod h264;
pub mod h265;
pub mod mjpeg;

/// Codec-specific RTP packetizer trait.
///
/// Each supported codec (H.264, H.265, MJPEG) implements this trait,
/// providing packetization logic and SDP attribute generation.
///
/// The generic RTP header is handled by [`rtp::RtpHeader`] — packetizers
/// compose it rather than reimplementing header logic.
pub trait Packetizer: Send {
    /// Packetize raw encoded data (e.g. Annex B bitstream) into RTP packets.
    /// Each returned `Vec<u8>` is a complete RTP packet (12-byte header + payload).
    fn packetize(&mut self, encoded_data: &[u8], timestamp_increment: u32) -> Vec<Vec<u8>>;

    /// Codec name for the SDP rtpmap attribute (e.g. "H264", "H265").
    fn codec_name(&self) -> &'static str;

    /// RTP clock rate in Hz (typically 90000 for video).
    fn clock_rate(&self) -> u32;

    /// RTP payload type number.
    fn payload_type(&self) -> u8;

    /// SDP media-level attributes for this codec (without "a=" prefix).
    /// Example: `vec!["fmtp:96 packetization-mode=1"]`
    fn sdp_attributes(&self) -> Vec<String>;

    /// Next RTP sequence number.
    fn next_sequence(&self) -> u16;

    /// Next RTP timestamp.
    fn next_rtp_timestamp(&self) -> u32;
}
