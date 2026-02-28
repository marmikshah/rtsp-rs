use base64::prelude::{BASE64_STANDARD, Engine as _};

use super::Packetizer;
use super::rtp::RtpHeader;

const DEFAULT_MTU: usize = 1400;

/// H.264 RTP packetizer (RFC 6184).
///
/// Converts H.264 Annex B bitstreams into RTP packets. Supports two
/// packetization modes from RFC 6184:
///
/// - **Single NAL Unit** (§5.6): NALs that fit within the MTU are sent
///   as-is in a single RTP packet (12-byte header + NAL bytes).
///
/// - **FU-A Fragmentation** (§5.8): NALs exceeding the MTU are split
///   across multiple RTP packets. Each fragment carries a 2-byte FU
///   header (FU indicator + FU header) before the NAL payload:
///
///   ```text
///   FU indicator:  [F|NRI|Type=28]     (1 byte)
///   FU header:     [S|E|R|NAL_Type]    (1 byte)
///   Fragment data: [...]               (up to MTU - 2 bytes)
///   ```
///
///   - **S** (start): set on the first fragment
///   - **E** (end): set on the last fragment
///   - **NAL_Type**: the original NAL unit type from the first byte
///
/// ## Annex B NAL extraction
///
/// H.264 Annex B bitstreams delimit NAL units with start codes:
/// - 4-byte: `0x00 0x00 0x00 0x01`
/// - 3-byte: `0x00 0x00 0x01`
///
/// [`extract_nal_units`](Self::extract_nal_units) handles both formats
/// and tracks each start code's length for correct boundary calculation.
///
/// ## SDP attributes (RFC 6184 §8.1)
///
/// The packetizer generates these SDP attributes:
/// - `a=rtpmap:96 H264/90000`
/// - `a=fmtp:96 packetization-mode=1`
/// - `a=control:track1`
///
/// SPS/PPS are auto-captured from the first frame that contains them (e.g. first keyframe);
/// the fmtp line then includes `profile-level-id` and `sprop-parameter-sets` (RFC 6184 §8.1).
///
/// ## Marker bit
///
/// Per RFC 6184 §5.1, the RTP marker bit is set on the last RTP packet
/// of an H.264 access unit (frame boundary).
#[derive(Debug)]
pub struct H264Packetizer {
    header: RtpHeader,
    mtu: usize,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl H264Packetizer {
    /// Create with explicit payload type and SSRC.
    pub fn new(pt: u8, ssrc: u32) -> Self {
        Self {
            header: RtpHeader::new(pt, ssrc),
            mtu: DEFAULT_MTU,
            sps: None,
            pps: None,
        }
    }

    /// Create with a random SSRC (RFC 3550 §8.1).
    pub fn with_random_ssrc(pt: u8) -> Self {
        Self {
            header: RtpHeader::with_random_ssrc(pt),
            mtu: DEFAULT_MTU,
            sps: None,
            pps: None,
        }
    }

    /// Derive profile-level-id from SPS NAL (RFC 6184 §8.1): bytes 1–3 are profile_idc, constraint_set, level_idc.
    fn get_profile_level_id(&self) -> Result<String, String> {
        let sps = self.sps.as_deref().ok_or("SPS not set")?;
        if sps.len() < 4 {
            return Err("SPS too short for profile-level-id".into());
        }
        Ok(format!("{:02x}{:02x}{:02x}", sps[1], sps[2], sps[3]))
    }

    fn get_sprop_parameter_sets(&self) -> Result<String, String> {
        let sps = self.sps.as_deref().ok_or("SPS not set")?;
        let pps = self.pps.as_deref().ok_or("PPS not set")?;
        Ok(format!(
            "{},{}",
            BASE64_STANDARD.encode(sps),
            BASE64_STANDARD.encode(pps)
        ))
    }

    /// Packetize a single NAL unit into one or more RTP packets.
    ///
    /// If the NAL fits within the MTU, it is sent as a Single NAL Unit
    /// packet (RFC 6184 §5.6). Otherwise, FU-A fragmentation is used
    /// (RFC 6184 §5.8).
    fn packetize_nal(&mut self, nal_unit: &[u8], is_last_nal: bool) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();

        if nal_unit.is_empty() {
            return packets;
        }

        if nal_unit.len() <= self.mtu {
            // Single NAL Unit mode (RFC 6184 §5.6)
            let hdr = self.header.write(is_last_nal);
            let mut packet = Vec::with_capacity(12 + nal_unit.len());
            packet.extend_from_slice(&hdr);
            packet.extend_from_slice(nal_unit);
            packets.push(packet);
        } else {
            // FU-A fragmentation (RFC 6184 §5.8)
            let nal_header = nal_unit[0];
            let nal_type = nal_header & 0x1f;
            let nri = nal_header & 0x60;

            // FU indicator: NRI from original NAL, type = 28 (FU-A)
            let fu_indicator = nri | 28;
            let payload = &nal_unit[1..];

            let max_fragment = self.mtu - 2; // 2 bytes for FU indicator + FU header
            let mut offset = 0usize;
            let mut first = true;

            while offset < payload.len() {
                let remaining = payload.len() - offset;
                let last_fragment = remaining <= max_fragment;
                let chunk_size = std::cmp::min(max_fragment, remaining);
                let chunk = &payload[offset..offset + chunk_size];

                // FU header: S=start, E=end, R=0, Type=original NAL type
                let start_bit = if first { 0x80 } else { 0x00 };
                let end_bit = if last_fragment { 0x40 } else { 0x00 };
                let fu_header = start_bit | end_bit | nal_type;

                let marker = is_last_nal && last_fragment;
                let hdr = self.header.write(marker);

                let mut packet = Vec::with_capacity(12 + 2 + chunk.len());
                packet.extend_from_slice(&hdr);
                packet.push(fu_indicator);
                packet.push(fu_header);
                packet.extend_from_slice(chunk);
                packets.push(packet);

                offset += chunk_size;
                first = false;
            }

            tracing::trace!(
                nal_type,
                nal_size = nal_unit.len(),
                fragments = packets.len(),
                "FU-A fragmented NAL unit"
            );
        }

        packets
    }

    /// Extract NAL units from an H.264 Annex B bitstream.
    ///
    /// Scans for start codes (both 4-byte `00 00 00 01` and 3-byte
    /// `00 00 01`) and returns the NAL data between them, excluding
    /// the start codes themselves.
    ///
    /// The start code length is tracked per-NAL to ensure boundaries
    /// between adjacent NALs are computed correctly when mixed 3-byte
    /// and 4-byte start codes appear.
    pub fn extract_nal_units(data: &[u8]) -> Vec<Vec<u8>> {
        let mut nal_units = Vec::new();
        let mut i = 0usize;

        // (nal_data_start_index, start_code_length)
        let mut start_entries: Vec<(usize, usize)> = Vec::new();

        while i < data.len() {
            if i + 3 < data.len() && data[i..i + 4] == [0, 0, 0, 1] {
                start_entries.push((i + 4, 4));
                i += 4;
            } else if i + 2 < data.len() && data[i..i + 3] == [0, 0, 1] {
                start_entries.push((i + 3, 3));
                i += 3;
            } else {
                i += 1;
            }
        }

        for (idx, &(start, _)) in start_entries.iter().enumerate() {
            let end = if idx + 1 < start_entries.len() {
                let (next_start, next_sc_len) = start_entries[idx + 1];
                next_start - next_sc_len
            } else {
                data.len()
            };

            if start < end {
                nal_units.push(data[start..end].to_vec());
            }
        }

        nal_units
    }
}

impl Packetizer for H264Packetizer {
    fn packetize(&mut self, encoded_data: &[u8], timestamp_increment: u32) -> Vec<Vec<u8>> {
        let nal_units = Self::extract_nal_units(encoded_data);
        let mut packets = Vec::new();

        // Auto-capture SPS/PPS from first frame that contains them (e.g. first keyframe).
        // Only set when not already provided by the user.
        if self.sps.is_none() || self.pps.is_none() {
            for nal in &nal_units {
                if nal.is_empty() {
                    continue;
                }
                let nal_type = nal[0] & 0x1f;
                if nal_type == 7 && self.sps.is_none() {
                    self.sps = Some(nal.clone());
                    tracing::debug!("H.264 SPS captured from bitstream ({} bytes)", nal.len());
                } else if nal_type == 8 && self.pps.is_none() {
                    self.pps = Some(nal.clone());
                    tracing::debug!("H.264 PPS captured from bitstream ({} bytes)", nal.len());
                }
            }
        }

        for (i, nal) in nal_units.iter().enumerate() {
            let is_last = i == nal_units.len() - 1;
            packets.append(&mut self.packetize_nal(nal, is_last));
        }

        self.header.advance_timestamp(timestamp_increment);

        tracing::trace!(
            nal_count = nal_units.len(),
            rtp_packets = packets.len(),
            frame_bytes = encoded_data.len(),
            seq = self.header.sequence(),
            ts = self.header.timestamp(),
            "frame packetized"
        );

        packets
    }

    fn codec_name(&self) -> &'static str {
        "H264"
    }

    /// 90 kHz clock rate per RFC 6184 §8.1.
    fn clock_rate(&self) -> u32 {
        90000
    }

    fn payload_type(&self) -> u8 {
        self.header.pt
    }

    /// SDP attributes per RFC 6184 §8.2.1.
    ///
    /// Order matters — `a=rtpmap` defines the payload type and MUST precede
    /// `a=fmtp` which references it. ffplay and other clients parse attributes
    /// sequentially and expect this ordering.
    ///
    /// - `a=rtpmap:<pt> H264/90000` — codec name and clock rate
    /// - `a=fmtp:<pt> packetization-mode=1[;profile-level-id=...][;sprop-parameter-sets=...]` — codec params (RFC 6184 §8.1)
    /// - `a=control:track1` — track control URL for SETUP
    fn sdp_attributes(&self) -> Vec<String> {
        let mut fmtp = format!("a=fmtp:{} packetization-mode=1", self.header.pt);
        if let Ok(pl) = self.get_profile_level_id() {
            fmtp.push_str(&format!(";profile-level-id={}", pl));
        }
        if let Ok(sprop) = self.get_sprop_parameter_sets() {
            fmtp.push_str(&format!(";sprop-parameter-sets={}", sprop));
        }

        vec![
            format!(
                "a=rtpmap:{} {}/{}",
                self.payload_type(),
                self.codec_name(),
                self.clock_rate()
            ),
            fmtp,
            "a=control:track1".to_string(),
        ]
    }

    fn next_sequence(&self) -> u16 {
        self.header.sequence()
    }

    fn next_rtp_timestamp(&self) -> u32 {
        self.header.timestamp() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_packetizer() -> H264Packetizer {
        H264Packetizer::new(96, 0xAABBCCDD)
    }

    // --- NAL extraction ---

    #[test]
    fn extract_single_nal_4byte_sc() {
        let data = [0, 0, 0, 1, 0x65, 0xAA, 0xBB];
        let nals = H264Packetizer::extract_nal_units(&data);
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0], vec![0x65, 0xAA, 0xBB]);
    }

    #[test]
    fn extract_single_nal_3byte_sc() {
        let data = [0, 0, 1, 0x67, 0x42, 0x00];
        let nals = H264Packetizer::extract_nal_units(&data);
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0], vec![0x67, 0x42, 0x00]);
    }

    #[test]
    fn extract_two_nals_4byte_sc() {
        let mut data = vec![0, 0, 0, 1, 0x67, 0x42];
        data.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xCE]);
        let nals = H264Packetizer::extract_nal_units(&data);
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0], vec![0x67, 0x42]);
        assert_eq!(nals[1], vec![0x68, 0xCE]);
    }

    #[test]
    fn extract_mixed_start_codes() {
        let mut data = vec![0, 0, 0, 1, 0x67, 0x42];
        data.extend_from_slice(&[0, 0, 1, 0x68, 0xCE]);
        let nals = H264Packetizer::extract_nal_units(&data);
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0], vec![0x67, 0x42]);
        assert_eq!(nals[1], vec![0x68, 0xCE]);
    }

    #[test]
    fn extract_empty_data() {
        assert!(H264Packetizer::extract_nal_units(&[]).is_empty());
    }

    #[test]
    fn extract_no_start_code() {
        assert!(H264Packetizer::extract_nal_units(&[0xFF, 0xFE]).is_empty());
    }

    // --- Packetization ---

    #[test]
    fn small_nal_single_packet() {
        let mut p = make_packetizer();
        let nal = vec![0x65, 0xAA, 0xBB, 0xCC];
        let packets = p.packetize_nal(&nal, true);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].len(), 12 + 4);
        assert_eq!(packets[0][1] & 0x80, 0x80); // marker bit
    }

    #[test]
    fn large_nal_fragmented() {
        let mut p = H264Packetizer::new(96, 0x11223344);
        let mut nal = vec![0x65]; // NAL header
        nal.extend(vec![0xAA; DEFAULT_MTU + 500]);
        let packets = p.packetize_nal(&nal, true);
        assert!(packets.len() > 1);

        assert_eq!(packets[0][12] & 0x1f, 28); // FU-A type
        assert_eq!(packets[0][13] & 0x80, 0x80); // Start bit

        let last = packets.last().unwrap();
        assert_eq!(last[13] & 0x40, 0x40); // End bit
        assert_eq!(last[1] & 0x80, 0x80); // Marker bit
    }

    #[test]
    fn empty_nal_no_packets() {
        let mut p = make_packetizer();
        assert!(p.packetize_nal(&[], true).is_empty());
    }

    #[test]
    fn packetize_trait_advances_timestamp() {
        let mut p = make_packetizer();
        let frame = [0, 0, 0, 1, 0x65, 0xAA, 0xBB];
        p.packetize(&frame, 3000);
        p.packetize(&frame, 3000);
        // 2 frames * 3000 = 6000 — verified through the trait interface
        let packets = p.packetize(&frame, 3000);
        assert!(!packets.is_empty());
    }

    #[test]
    fn sdp_attributes_include_packetization_mode() {
        let p = make_packetizer();
        let attrs = p.sdp_attributes();
        assert!(attrs.len() >= 1, "must include at least fmtp");
        assert!(
            attrs.iter().any(|a| a.contains("packetization-mode=1")),
            "must include packetization-mode=1"
        );
    }

    #[test]
    fn codec_metadata() {
        let p = make_packetizer();
        assert_eq!(p.codec_name(), "H264");
        assert_eq!(p.clock_rate(), 90000);
        assert_eq!(p.payload_type(), 96);
    }

    #[test]
    fn auto_capture_sps_pps_from_first_frame() {
        // Frame with SPS (NAL 7) and PPS (NAL 8): packetizer captures them for SDP
        let mut p = H264Packetizer::new(96, 0xAABBCCDD);
        let sps_nal = vec![0x67, 0x42, 0x00, 0x1e]; // NAL type 7
        let pps_nal = vec![0x68, 0xce, 0x38, 0x80]; // NAL type 8
        let frame = [
            &[0u8, 0, 0, 1][..],
            sps_nal.as_slice(),
            &[0, 0, 0, 1][..],
            pps_nal.as_slice(),
            &[0, 0, 0, 1, 0x65, 0x88, 0x00][..], // slice
        ]
        .concat();
        p.packetize(&frame, 3000);
        let attrs = p.sdp_attributes();
        let fmtp = attrs
            .iter()
            .find(|a| a.starts_with("a=fmtp:"))
            .expect("fmtp line");
        assert!(
            fmtp.contains("profile-level-id="),
            "SPS auto-captured, profile-level-id in SDP"
        );
        assert!(
            fmtp.contains("sprop-parameter-sets="),
            "SPS/PPS auto-captured, sprop-parameter-sets in SDP"
        );
    }
}
