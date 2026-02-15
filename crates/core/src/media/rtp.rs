use rand::Rng;

/// Generic RTP fixed header builder (RFC 3550 §5.1).
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |V=2|P|X|  CC   |M|     PT      |       Sequence Number         |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                           Timestamp                           |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                             SSRC                              |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
///
/// This struct is shared by all codec packetizers. It manages:
/// - **Sequence number**: 16-bit, wrapping — incremented on every packet.
/// - **Timestamp**: stored as u64 internally to avoid wrapping arithmetic
///   during duration calculations; the lower 32 bits are written to the wire.
/// - **SSRC**: randomly generated per RFC 3550 §8.1 to avoid collisions.
///
/// Version is always 2. Padding, extension, and CSRC count are always 0.
#[derive(Debug)]
pub struct RtpHeader {
    /// RTP payload type (7-bit, RFC 3551).
    pub pt: u8,
    /// Synchronization source identifier (RFC 3550 §8.1).
    pub ssrc: u32,
    sequence: u16,
    timestamp: u64,
}

impl RtpHeader {
    /// Create a new RTP header state with explicit SSRC.
    pub fn new(pt: u8, ssrc: u32) -> Self {
        tracing::debug!(
            pt,
            ssrc = format_args!("{:#010X}", ssrc),
            "RTP header state created"
        );
        Self {
            pt,
            ssrc,
            sequence: 0,
            timestamp: 0,
        }
    }

    /// Create with a random SSRC.
    ///
    /// Per RFC 3550 §8.1, the SSRC should be chosen randomly to minimize
    /// the probability of collisions between independent sessions.
    pub fn with_random_ssrc(pt: u8) -> Self {
        let ssrc = rand::rng().random::<u32>();
        Self::new(pt, ssrc)
    }

    /// Current sequence number (before the next [`write`](Self::write) call).
    pub fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Current timestamp (internal u64 representation).
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Serialize a 12-byte RTP fixed header and advance the sequence number.
    ///
    /// The `marker` bit (RFC 3550 §5.1) signals the last packet of a frame.
    /// For H.264, it is set on the last RTP packet of an access unit
    /// (RFC 6184 §5.1).
    pub fn write(&mut self, marker: bool) -> [u8; 12] {
        let first_byte: u8 = 2 << 6;
        let second_byte: u8 = ((marker as u8) << 7) | self.pt;

        let mut header = [0u8; 12];
        header[0] = first_byte;
        header[1] = second_byte;
        header[2..4].copy_from_slice(&self.sequence.to_be_bytes());
        header[4..8].copy_from_slice(&(self.timestamp as u32).to_be_bytes());
        header[8..12].copy_from_slice(&self.ssrc.to_be_bytes());

        self.sequence = self.sequence.wrapping_add(1);
        header
    }

    /// Advance the RTP timestamp by the given increment.
    ///
    /// For video at 90 kHz clock rate, the increment per frame is
    /// `90000 / fps` (e.g. 3000 for 30 fps, 3600 for 25 fps).
    pub fn advance_timestamp(&mut self, increment: u32) {
        self.timestamp = self.timestamp.wrapping_add(increment as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header() -> RtpHeader {
        RtpHeader::new(96, 0xAABBCCDD)
    }

    #[test]
    fn version_is_2() {
        let mut h = make_header();
        let buf = h.write(false);
        assert_eq!(buf[0] >> 6, 2);
    }

    #[test]
    fn marker_bit() {
        let mut h = make_header();
        let no_marker = h.write(false);
        assert_eq!(no_marker[1] & 0x80, 0);

        let with_marker = h.write(true);
        assert_eq!(with_marker[1] & 0x80, 0x80);
    }

    #[test]
    fn payload_type() {
        let mut h = make_header();
        let buf = h.write(false);
        assert_eq!(buf[1] & 0x7f, 96);
    }

    #[test]
    fn sequence_increments() {
        let mut h = make_header();
        let b1 = h.write(false);
        let seq1 = u16::from_be_bytes([b1[2], b1[3]]);
        let b2 = h.write(false);
        let seq2 = u16::from_be_bytes([b2[2], b2[3]]);
        assert_eq!(seq2, seq1 + 1);
    }

    #[test]
    fn sequence_wraps() {
        let mut h = make_header();
        h.sequence = u16::MAX;
        let buf = h.write(false);
        let seq = u16::from_be_bytes([buf[2], buf[3]]);
        assert_eq!(seq, u16::MAX);
        assert_eq!(h.sequence(), 0);
    }

    #[test]
    fn ssrc_written() {
        let mut h = make_header();
        let buf = h.write(false);
        let ssrc = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        assert_eq!(ssrc, 0xAABBCCDD);
    }

    #[test]
    fn timestamp_advance() {
        let mut h = make_header();
        h.advance_timestamp(3000);
        assert_eq!(h.timestamp(), 3000);
        h.advance_timestamp(3000);
        assert_eq!(h.timestamp(), 6000);
    }

    #[test]
    fn random_ssrc_differs() {
        let h1 = RtpHeader::with_random_ssrc(96);
        let h2 = RtpHeader::with_random_ssrc(96);
        assert_ne!(h1.ssrc, h2.ssrc);
    }
}
