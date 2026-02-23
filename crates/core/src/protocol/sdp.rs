use std::{fmt::format, net::Ipv4Addr};

use crate::media::Packetizer;



/// Generate an SDP session description for the given packetizer.
///
/// Produces RFC 8866 compliant SDP with tolerance for RFC 4566
/// [`Packetizer::sdp_attributes`] implementation.
pub fn generate_sdp(packetizer: &dyn Packetizer, ip: &str, session_id: &str, session_version: &str, username: &str, session_name: &str) -> String {
    let pt = packetizer.payload_type();
    let clock = packetizer.clock_rate();
    let codec = packetizer.codec_name();

    let mut sdp: Vec<String> = Vec::new();
    // 1. v= (Protocol Version)
    sdp.push("v=0".to_string());

    // 2. o= (Origin)
    sdp.push(format!("o={} {} {} IN IP4 {}", username, session_id, session_version, ip));

    // 3. s= (Session Name)
    sdp.push(format!("s={}", session_name));

    // 4. c= (Connnection Data - placed at session level for RTSP)
    sdp.push("c=IN IP4 0.0.0.0".to_string());

    // 5. t= (Timing - 0 0 means a live, continuous stream)
    sdp.push("t=0 0".to_string());

    //6. a= (Session Attributes)
    sdp.extend_from_slice(&packetizer.sdp_attributes()[0..]);

    // 7. m= (Media Description)
    // TODO: Add Audio Support
    sdp.push(format!("m=video 0 RTP/AVP {}", &packetizer.payload_type()));

    // Final has to be a \r\n
    sdp.push("\r\n".to_string());

    sdp.join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::h264::H264Packetizer;

    #[test]
    fn generates_h264_sdp() {
        let p = H264Packetizer::new(96, 0x12345678);
        let sdp = generate_sdp(&p);
        assert!(sdp.contains("v=0\r\n"));
        assert!(sdp.contains("a=rtpmap:96 H264/90000\r\n"));
        assert!(sdp.contains("a=fmtp:96 packetization-mode=1\r\n"));
        assert!(sdp.contains("a=control:track1\r\n"));
    }
}
