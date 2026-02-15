//! Error types for the RTSP server library.

use std::fmt;

/// Errors that can occur in the RTSP server library.
///
/// Variants map to specific failure modes across the stack:
///
/// - **Protocol**: [`Parse`](Self::Parse) — malformed RTSP messages.
/// - **Transport**: [`Io`](Self::Io) — socket/network failures.
/// - **Session**: [`SessionNotFound`](Self::SessionNotFound),
///   [`SessionNotPlaying`](Self::SessionNotPlaying),
///   [`TransportNotConfigured`](Self::TransportNotConfigured).
/// - **Server**: [`NotStarted`](Self::NotStarted),
///   [`AlreadyRunning`](Self::AlreadyRunning).
/// - **Mount**: [`MountNotFound`](Self::MountNotFound).
#[derive(Debug, thiserror::Error)]
pub enum RtspError {
    /// Underlying I/O or socket error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// No session with the given ID exists in the [`SessionManager`](crate::session::SessionManager).
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// SETUP has not been completed for this session (no UDP ports negotiated).
    #[error("transport not configured for session: {0}")]
    TransportNotConfigured(String),

    /// Attempted to send media to a session that is not in the Playing state.
    #[error("session not in playing state: {0}")]
    SessionNotPlaying(String),

    /// [`Server::start`](crate::Server::start) has not been called yet.
    #[error("server not started")]
    NotStarted,

    /// [`Server::start`](crate::Server::start) was called while already running.
    #[error("server already running")]
    AlreadyRunning,

    /// Failed to parse an RTSP request message (RFC 2326 §6).
    #[error("RTSP parse error: {kind}")]
    Parse { kind: ParseErrorKind },

    /// Server-side UDP port allocation exhausted the 5000–65534 range.
    #[error("port range exhausted (tried to allocate beyond u16 range)")]
    PortRangeExhausted,

    /// No mount registered at the requested path.
    #[error("mount not found: {0}")]
    MountNotFound(String),
}

/// Specific kind of RTSP parse failure.
#[derive(Debug)]
pub enum ParseErrorKind {
    /// Input was empty (no request line).
    EmptyRequest,
    /// Request line did not have the expected `Method URI Version` format.
    InvalidRequestLine,
    /// A header line did not contain a colon separator.
    InvalidHeader,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequest => write!(f, "empty request"),
            Self::InvalidRequestLine => write!(f, "invalid request line"),
            Self::InvalidHeader => write!(f, "invalid header"),
        }
    }
}

/// Convenience alias for `Result<T, RtspError>`.
pub type Result<T> = std::result::Result<T, RtspError>;
