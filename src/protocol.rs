#[derive(Debug)]
pub struct RtspRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
}

impl RtspRequest {
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    // Command Sequence (CSeq) is used to number and order RTSP requests & responses
    pub fn cseq(&self) -> Option<&str> {
        self.get_header("CSeq")
    }
}

#[derive(Debug)]
pub enum ParseError {
    EmptyRequest,
    InvalidRequestLine,
    InvalidHeader,
}
pub fn parse_request(raw: &str) -> Result<RtspRequest, ParseError> {
    let mut lines = raw.lines();

    let request_line = lines.next().ok_or(ParseError::EmptyRequest)?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() != 3 {
        return Err(ParseError::InvalidRequestLine);
    }

    let method = parts[0].to_string();
    let uri = parts[1].to_string();
    let version = parts[2].to_string();

    let mut headers = Vec::new();

    for line in lines {
        if line.is_empty() {
            break;
        }

        let colon_pos = line.find(':').ok_or(ParseError::InvalidHeader)?;
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

#[derive(Debug, Clone)]
pub struct TransportHeader {
    pub client_rtp_port: u16,
    pub client_rtcp_port: u16,
}

pub fn parse_transport_header(header: &str) -> Option<TransportHeader> {
    for part in header.split(';') {
        let part = part.trim();
        if part.starts_with("client_port=") {
            let ports = &part["client_port=".len()..];
            let port_parts: Vec<&str> = ports.split("-").collect();

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

pub struct RtspResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl RtspResponse {
    pub fn new(status_code: u16, status_text: &str) -> Self {
        RtspResponse {
            status_code,
            status_text: status_text.to_string(),
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn ok() -> Self {
        Self::new(200, "OK")
    }
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }
    pub fn bad_request() -> Self {
        Self::new(400, "Bad request")
    }

    pub fn add_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

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
