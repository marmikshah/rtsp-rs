/// An RTSP response (RFC 2326 §7).
///
/// Serializes to the standard text format:
///
/// ```text
/// RTSP/1.0 200 OK\r\n
/// CSeq: 1\r\n
/// Content-Type: application/sdp\r\n
/// Content-Length: 142\r\n
/// \r\n
/// v=0\r\n...
/// ```
///
/// Uses a builder pattern — chain [`add_header`](Self::add_header) and
/// [`with_body`](Self::with_body), then call [`serialize`](Self::serialize).
/// `Content-Length` is computed automatically when a body is present.
#[must_use]
pub struct RtspResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// Server identification string included in every RTSP response
/// per RFC 2326 §12.36.
pub const SERVER_AGENT: &str = "rtsp-rs/0.1";

impl RtspResponse {
    pub fn new(status_code: u16, status_text: &str) -> Self {
        RtspResponse {
            status_code,
            status_text: status_text.to_string(),
            headers: vec![("Server".to_string(), SERVER_AGENT.to_string())],
            body: None,
        }
    }

    /// 200 OK — success (RFC 2326 §7.1.1).
    pub fn ok() -> Self {
        Self::new(200, "OK")
    }

    /// 404 Not Found — the requested resource does not exist.
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }

    /// 400 Bad Request — malformed or missing required header.
    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request")
    }

    pub fn add_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    /// Serialize to the RTSP text wire format.
    ///
    /// If a body is present, `Content-Length` is appended automatically
    /// (RFC 2326 §12.14).
    pub fn serialize(&self) -> String {
        let mut response = format!("RTSP/1.0 {} {}\r\n", self.status_code, self.status_text);

        for (name, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", name, value));
        }

        if let Some(body) = &self.body {
            response.push_str(&format!("Content-Length: {}\r\n", body.len()));
            response.push_str("\r\n");
            response.push_str(body);
        } else {
            response.push_str("\r\n");
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_no_body() {
        let resp = RtspResponse::ok()
            .add_header("CSeq", "1")
            .add_header("Public", "OPTIONS");
        let s = resp.serialize();
        assert!(s.starts_with("RTSP/1.0 200 OK\r\n"));
        assert!(s.contains("Server: rtsp-rs/0.1\r\n"));
        assert!(s.contains("CSeq: 1\r\n"));
        assert!(s.contains("Public: OPTIONS\r\n"));
        assert!(s.ends_with("\r\n"));
    }

    #[test]
    fn serialize_with_body() {
        let resp = RtspResponse::ok()
            .add_header("CSeq", "2")
            .with_body("v=0\r\n".to_string());
        let s = resp.serialize();
        assert!(s.contains("Server: rtsp-rs/0.1\r\n"));
        assert!(s.contains("Content-Length: 5\r\n"));
        assert!(s.ends_with("v=0\r\n"));
    }

    #[test]
    fn not_found_response() {
        let resp = RtspResponse::not_found().add_header("CSeq", "5");
        assert_eq!(resp.status_code, 404);
        let s = resp.serialize();
        assert!(s.starts_with("RTSP/1.0 404 Not Found\r\n"));
        assert!(s.contains("Server: rtsp-rs/0.1\r\n"));
    }
}
