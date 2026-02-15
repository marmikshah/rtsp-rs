//! SDP (Session Description Protocol) generation (RFC 4566 / RFC 8866).
//!
//! Produces the SDP body returned by DESCRIBE responses. The format:
//!
//! ```text
//! v=0                                          ← protocol version
//! o=<user> <sess-id> <sess-ver> IN IP4 <addr>  ← origin
//! s=<session-name>                              ← session name
//! c=IN IP4 <addr>                               ← connection address
//! t=0 0                                         ← timing (live stream)
//! a=tool:rtsp-rs                                ← server software (§6)
//! a=sendonly                                    ← direction (§6)
//! m=video 0 RTP/AVP 96                          ← media description
//! a=rtpmap:96 H264/90000                        ← codec/clock rate
//! a=fmtp:96 packetization-mode=1                ← codec parameters
//! a=control:track1                              ← track control URL
//! ```
//!
//! All session/origin fields come from [`ServerConfig`](crate::ServerConfig)
//! so nothing is hardcoded.

use crate::mount::Mount;

/// Generate an SDP session description for the given mount.
///
/// When multi-track (audio+video) support is added, this will iterate
/// over the mount's tracks to produce multiple `m=` lines.
pub fn generate_sdp(
    mount: &Mount,
    ip: &str,
    session_id: &str,
    session_version: &str,
    username: &str,
    session_name: &str,
) -> String {
    let mut sdp: Vec<String> = Vec::new();

    sdp.push("v=0".to_string());
    sdp.push(format!(
        "o={} {} {} IN IP4 {}",
        username, session_id, session_version, ip
    ));
    sdp.push(format!("s={}", session_name));
    sdp.push(format!("c=IN IP4 {}", ip));
    sdp.push("t=0 0".to_string());
    sdp.push("a=tool:rtsp-rs".to_string());
    sdp.push("a=sendonly".to_string());
    sdp.push(format!("m=video 0 RTP/AVP {}", mount.payload_type()));
    sdp.extend_from_slice(&mount.sdp_attributes()[0..]);

    tracing::debug!("SDP: {}", sdp.join("\r\n"));

    format!("{}\r\n", sdp.join("\r\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::h264::H264Packetizer;

    #[test]
    fn generates_h264_sdp() {
        let mount = Mount::new("/stream", Box::new(H264Packetizer::new(96, 0x12345678)));
        let sdp = generate_sdp(
            &mount,
            "192.168.1.100",
            "1234567890",
            "1",
            "server",
            "Test Session",
        );
        assert!(sdp.contains("v=0\r\n"));
        assert!(sdp.contains("o=server 1234567890 1 IN IP4 192.168.1.100\r\n"));
        assert!(sdp.contains("s=Test Session\r\n"));
        assert!(
            sdp.contains("c=IN IP4 192.168.1.100\r\n"),
            "c= must use configured IP, not 0.0.0.0"
        );
        assert!(
            sdp.contains("a=tool:rtsp-rs\r\n"),
            "SDP must include tool attribute"
        );
        assert!(
            sdp.contains("a=sendonly\r\n"),
            "SDP must include sendonly direction"
        );
        assert!(
            sdp.contains("a=rtpmap:96 H264/90000\r\n"),
            "SDP must include valid rtpmap"
        );
        assert!(sdp.contains("a=fmtp:96 packetization-mode=1\r\n"));
        assert!(sdp.contains("a=control:track1\r\n"));

        // Verify ordering: rtpmap must come before fmtp (RFC 6184 §8.2.1)
        let rtpmap_idx = sdp.find("a=rtpmap").expect("SDP must include rtpmap");
        let fmtp_idx = sdp.find("a=fmtp").expect("SDP must include fmtp");
        assert!(
            rtpmap_idx < fmtp_idx,
            "a=rtpmap must precede a=fmtp per RFC 6184"
        );

        // Session-level attrs must come before media section
        let sendonly_idx = sdp.find("a=sendonly").expect("SDP must include sendonly");
        let m_idx = sdp.find("m=video").expect("SDP must include media section");
        assert!(
            sendonly_idx < m_idx,
            "session-level attrs must precede m= line"
        );

        assert!(fmtp_idx > m_idx, "media attributes must follow m=video");
        assert!(sdp.ends_with("\r\n"), "SDP must end with CRLF");
    }
}
