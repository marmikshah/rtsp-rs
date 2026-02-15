use std::net::SocketAddr;

/// Negotiated RTP/RTCP transport parameters for a session (RFC 2326 §12.39).
///
/// Created during SETUP from the client's `Transport` header and the
/// server's allocated port pair. Used to address UDP packets.
///
/// ## Wire format example
///
/// ```text
/// Client → Server:
///   Transport: RTP/AVP;unicast;client_port=8000-8001
///
/// Server → Client:
///   Transport: RTP/AVP;unicast;client_port=8000-8001;server_port=5000-5001
/// ```
///
/// The server sends RTP to `client_addr:client_rtp_port` and (future)
/// RTCP to `client_addr:client_rtcp_port`.
#[derive(Debug, Clone)]
pub struct Transport {
    /// Client's RTP receive port.
    pub client_rtp_port: u16,
    /// Client's RTCP receive port (typically `client_rtp_port + 1`).
    pub client_rtcp_port: u16,
    /// Server's RTP send port (advertised to client, not actually bound).
    pub server_rtp_port: u16,
    /// Server's RTCP port (advertised to client, not actually bound).
    pub server_rtcp_port: u16,
    /// Full socket address for RTP delivery (`client_ip:client_rtp_port`).
    pub client_addr: SocketAddr,
}

/// Parsed client-side transport info from the RTSP `Transport` header.
///
/// Extracts the `client_port=RTP-RTCP` pair from the header value.
/// Currently only handles `RTP/AVP;unicast` — interleaved TCP and
/// multicast are not yet supported (see Issues #14 and RFC 2326 §12.39).
#[derive(Debug, Clone)]
pub struct TransportHeader {
    /// Client's requested RTP port.
    pub client_rtp_port: u16,
    /// Client's requested RTCP port.
    pub client_rtcp_port: u16,
}

impl TransportHeader {
    /// Parse the `Transport` header value (RFC 2326 §12.39).
    ///
    /// Looks for `client_port=RTP-RTCP` among semicolon-separated parameters.
    ///
    /// ## Examples
    ///
    /// ```
    /// use rtsp::session::transport::TransportHeader;
    ///
    /// let th = TransportHeader::parse("RTP/AVP;unicast;client_port=8000-8001").unwrap();
    /// assert_eq!(th.client_rtp_port, 8000);
    /// assert_eq!(th.client_rtcp_port, 8001);
    ///
    /// assert!(TransportHeader::parse("RTP/AVP;unicast").is_none());
    /// ```
    pub fn parse(header: &str) -> Option<Self> {
        for part in header.split(';') {
            let part = part.trim();
            if let Some(ports) = part.strip_prefix("client_port=") {
                let port_parts: Vec<&str> = ports.split('-').collect();

                if port_parts.len() == 2 {
                    let rtp_port: u16 = port_parts[0].parse().ok()?;
                    let rtcp_port: u16 = port_parts[1].parse().ok()?;

                    return Some(TransportHeader {
                        client_rtp_port: rtp_port,
                        client_rtcp_port: rtcp_port,
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_transport() {
        let th = TransportHeader::parse("RTP/AVP;unicast;client_port=5000-5001").unwrap();
        assert_eq!(th.client_rtp_port, 5000);
        assert_eq!(th.client_rtcp_port, 5001);
    }

    #[test]
    fn parse_no_client_port() {
        assert!(TransportHeader::parse("RTP/AVP;unicast").is_none());
    }
}
