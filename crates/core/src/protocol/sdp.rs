use crate::media::Packetizer;

/// Generate an SDP session description for the given packetizer.
///
/// Produces RFC 8866 compliant SDP with tolerance for RFC 4566
/// [`Packetizer::sdp_attributes`] implementation.
///
/// All session/origin fields are taken from arguments so nothing is hardcoded (per config).
pub fn generate_sdp(
    packetizer: &dyn Packetizer,
    ip: &str,
    session_id: &str,
    session_version: &str,
    username: &str,
    session_name: &str,
) -> String {
    let mut sdp: Vec<String> = Vec::new();
    // 1. v= (Protocol Version)
    sdp.push("v=0".to_string());

    // 2. o= (Origin)
    sdp.push(format!("o={} {} {} IN IP4 {}", username, session_id, session_version, ip));

    // 3. s= (Session Name)
    sdp.push(format!("s={}", session_name));

    // 4. c= (Connection Data - session level; use configured IP, not hardcoded 0.0.0.0)
    sdp.push(format!("c=IN IP4 {}", ip));

    // 5. t= (Timing - 0 0 means a live, continuous stream)
    sdp.push("t=0 0".to_string());

    // 6. m= (Media Description)
    // TODO: Add Audio Support
    sdp.push(format!("m=video 0 RTP/AVP {}", &packetizer.payload_type()));

    // 7. a= (Media Attributes)
    sdp.extend_from_slice(&packetizer.sdp_attributes()[0..]);

    tracing::debug!("SDP: {}", sdp.join("\r\n"));

    // SDP body must end with CRLF for RTSP response bodies.
    format!("{}\r\n", sdp.join("\r\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::h264::H264Packetizer;

    #[test]
    fn generates_h264_sdp() {
        let p = H264Packetizer::new(96, 0x12345678);
        let sdp = generate_sdp(
            &p,
            "192.168.1.100",
            "1234567890",
            "1",
            "server",
            "Test Session",
        );
        assert!(sdp.contains("v=0\r\n"));
        assert!(sdp.contains("o=server 1234567890 1 IN IP4 192.168.1.100\r\n"));
        assert!(sdp.contains("s=Test Session\r\n"));
        assert!(sdp.contains("c=IN IP4 192.168.1.100\r\n"), "c= must use configured IP, not 0.0.0.0");
        // Attributes come from packetizer and must be RFC 6184 / RTP map format.
        assert!(sdp.contains("a=rtpmap:96 H264/90000\r\n"), "SDP must include valid rtpmap");
        assert!(sdp.contains("a=fmtp:96 packetization-mode=1\r\n"));
        assert!(sdp.contains("a=control:track1\r\n"));
        let m_idx = sdp.find("m=video").expect("SDP must include media section");
        let a_idx = sdp.find("a=fmtp").expect("SDP must include fmtp attribute");
        assert!(a_idx > m_idx, "media attributes must follow m=video");
        assert!(sdp.ends_with("\r\n"), "SDP must end with CRLF");
    }
}
