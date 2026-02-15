use crate::error::{ParseErrorKind, RtspError};

/// A parsed RTSP request (RFC 2326 §6).
///
/// RTSP requests follow HTTP/1.1 syntax:
///
/// ```text
/// Method SP Request-URI SP RTSP-Version CRLF
/// *(Header: Value CRLF)
/// CRLF
/// [body]
/// ```
///
/// Header lookup is case-insensitive per RFC 2326 §4.2.
///
/// Note: body parsing is not yet implemented (requires reading
/// `Content-Length` bytes after the blank line).
#[derive(Debug)]
pub struct RtspRequest {
    /// RTSP method (OPTIONS, DESCRIBE, SETUP, PLAY, etc.).
    pub method: String,
    /// Request-URI (e.g. `rtsp://host:port/stream/track1`).
    pub uri: String,
    /// Protocol version (expected: `RTSP/1.0`).
    pub version: String,
    /// Headers as ordered (name, value) pairs. Names are stored as-received;
    /// lookups via [`get_header`](Self::get_header) are case-insensitive.
    pub headers: Vec<(String, String)>,
}

impl RtspRequest {
    /// Parse an RTSP request from its text representation.
    ///
    /// Expects a complete request: request line, headers, and trailing blank
    /// line. Returns [`RtspError::Parse`] on malformed input.
    pub fn parse(raw: &str) -> crate::error::Result<Self> {
        let mut lines = raw.lines();

        let request_line = lines.next().ok_or(RtspError::Parse {
            kind: ParseErrorKind::EmptyRequest,
        })?;

        let parts: Vec<&str> = request_line.split_whitespace().collect();

        if parts.len() != 3 {
            return Err(RtspError::Parse {
                kind: ParseErrorKind::InvalidRequestLine,
            });
        }

        let method = parts[0].to_string();
        let uri = parts[1].to_string();
        let version = parts[2].to_string();

        if version != "RTSP/1.0" {
            tracing::warn!(version, "client sent non-RTSP/1.0 version");
        }

        let mut headers = Vec::new();

        for line in lines {
            if line.is_empty() {
                break;
            }

            let colon_pos = line.find(':').ok_or(RtspError::Parse {
                kind: ParseErrorKind::InvalidHeader,
            })?;

            let name = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();

            headers.push((name, value));
        }

        Ok(RtspRequest {
            method,
            uri,
            version,
            headers,
        })
    }

    /// Look up a header value by name (case-insensitive, per RFC 2326 §4.2).
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Returns the CSeq header value, which numbers and orders RTSP
    /// request/response pairs (RFC 2326 §12.17).
    ///
    /// Every RTSP request must include a CSeq, and the response must echo it.
    pub fn cseq(&self) -> Option<&str> {
        self.get_header("CSeq")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_options_request() {
        let raw = "OPTIONS rtsp://localhost:8554/test RTSP/1.0\r\nCSeq: 1\r\n\r\n";
        let req = RtspRequest::parse(raw).unwrap();
        assert_eq!(req.method, "OPTIONS");
        assert_eq!(req.uri, "rtsp://localhost:8554/test");
        assert_eq!(req.version, "RTSP/1.0");
        assert_eq!(req.cseq(), Some("1"));
    }

    #[test]
    fn parse_setup_with_transport() {
        let raw = "SETUP rtsp://localhost:8554/test/track1 RTSP/1.0\r\n\
                   CSeq: 3\r\n\
                   Transport: RTP/AVP;unicast;client_port=8000-8001\r\n\r\n";
        let req = RtspRequest::parse(raw).unwrap();
        assert_eq!(req.method, "SETUP");
        assert_eq!(req.cseq(), Some("3"));
        assert_eq!(
            req.get_header("Transport"),
            Some("RTP/AVP;unicast;client_port=8000-8001")
        );
    }

    #[test]
    fn parse_empty_request() {
        assert!(RtspRequest::parse("").is_err());
    }

    #[test]
    fn parse_invalid_request_line() {
        assert!(RtspRequest::parse("JUST_A_METHOD\r\n\r\n").is_err());
    }

    #[test]
    fn header_lookup_case_insensitive() {
        let raw = "OPTIONS rtsp://localhost RTSP/1.0\r\ncseq: 42\r\n\r\n";
        let req = RtspRequest::parse(raw).unwrap();
        assert_eq!(req.get_header("CSeq"), Some("42"));
        assert_eq!(req.get_header("cseq"), Some("42"));
        assert_eq!(req.get_header("CSEQ"), Some("42"));
    }
}
