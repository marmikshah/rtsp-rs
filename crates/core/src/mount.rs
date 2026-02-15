use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use crate::media::Packetizer;

pub const DEFAULT_MOUNT_PATH: &str = "/stream";

/// A named stream endpoint (e.g. `/stream`, `/camera1`).
///
/// Owns a packetizer for its codec and tracks which sessions are subscribed.
/// In the future, a mount may contain multiple tracks (video + audio).
pub struct Mount {
    path: String,
    packetizer: Mutex<Box<dyn Packetizer>>,
    session_ids: RwLock<Vec<String>>,
}

impl Mount {
    pub fn new(path: &str, packetizer: Box<dyn Packetizer>) -> Self {
        Self {
            path: path.to_string(),
            packetizer: Mutex::new(packetizer),
            session_ids: RwLock::new(Vec::new()),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Packetize raw encoded data into RTP packets using this mount's codec.
    pub fn packetize(&self, data: &[u8], timestamp_increment: u32) -> Vec<Vec<u8>> {
        self.packetizer.lock().packetize(data, timestamp_increment)
    }

    /// RTP payload type from the underlying packetizer.
    pub fn payload_type(&self) -> u8 {
        self.packetizer.lock().payload_type()
    }

    /// SDP media-level attributes (delegated to packetizer).
    pub fn sdp_attributes(&self) -> Vec<String> {
        self.packetizer.lock().sdp_attributes()
    }

    /// Codec clock rate in Hz.
    pub fn clock_rate(&self) -> u32 {
        self.packetizer.lock().clock_rate()
    }

    /// Next RTP sequence number (for RTP-Info header).
    pub fn next_sequence(&self) -> u16 {
        self.packetizer.lock().next_sequence()
    }

    /// Next RTP timestamp (for RTP-Info header).
    pub fn next_rtp_timestamp(&self) -> u32 {
        self.packetizer.lock().next_rtp_timestamp()
    }

    /// Subscribe a session to this mount (called during SETUP).
    pub fn subscribe(&self, session_id: &str) {
        let mut ids = self.session_ids.write();
        if !ids.iter().any(|id| id == session_id) {
            ids.push(session_id.to_string());
            tracing::debug!(mount = %self.path, session_id, "session subscribed");
        }
    }

    /// Unsubscribe a session from this mount (called during TEARDOWN or disconnect).
    pub fn unsubscribe(&self, session_id: &str) {
        let mut ids = self.session_ids.write();
        if let Some(pos) = ids.iter().position(|id| id == session_id) {
            ids.swap_remove(pos);
            tracing::debug!(mount = %self.path, session_id, "session unsubscribed");
        }
    }

    /// Returns the list of subscribed session IDs.
    pub fn subscribed_session_ids(&self) -> Vec<String> {
        self.session_ids.read().clone()
    }
}

/// Registry of named mount points, keyed by path.
///
/// Supports a "default" mount that acts as a fallback when the requested
/// URI path doesn't match any registered mount. This ensures that clients
/// requesting any path (e.g. `/test`, `/live`) will be served by the
/// default stream when only one mount is configured.
#[derive(Clone)]
pub struct MountRegistry {
    mounts: Arc<RwLock<HashMap<String, Arc<Mount>>>>,
    /// Path of the default (fallback) mount, used when no exact match is found.
    default_path: Arc<RwLock<Option<String>>>,
}

impl MountRegistry {
    pub fn new() -> Self {
        Self {
            mounts: Arc::new(RwLock::new(HashMap::new())),
            default_path: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a new mount point. Replaces any existing mount at the same path.
    pub fn add(&self, path: &str, packetizer: Box<dyn Packetizer>) -> Arc<Mount> {
        let mount = Arc::new(Mount::new(path, packetizer));
        self.mounts.write().insert(path.to_string(), mount.clone());
        tracing::info!(path, "mount registered");
        mount
    }

    /// Designate a mount path as the default fallback.
    ///
    /// When [`resolve_from_uri`](Self::resolve_from_uri) cannot find an
    /// exact match, it falls back to this mount. Typically set to
    /// [`DEFAULT_MOUNT_PATH`] by the server constructor.
    pub fn set_default(&self, path: &str) {
        *self.default_path.write() = Some(path.to_string());
    }

    /// Look up a mount by exact path.
    pub fn get(&self, path: &str) -> Option<Arc<Mount>> {
        self.mounts.read().get(path).cloned()
    }

    /// Resolve a mount from an RTSP URI.
    ///
    /// Tries an exact path match first. If no mount is found, falls back
    /// to the default mount (if one has been set via [`set_default`](Self::set_default)).
    ///
    /// This fallback ensures clients requesting any URI (e.g.
    /// `rtsp://host/test`) are served when only a single default stream
    /// is configured — matching the behavior of most RTSP server
    /// implementations.
    pub fn resolve_from_uri(&self, uri: &str) -> Option<Arc<Mount>> {
        let path = extract_mount_path(uri);
        self.get(path).or_else(|| {
            let default = self.default_path.read();
            default.as_ref().and_then(|p| self.get(p))
        })
    }

    /// Unsubscribe a session from all mounts (used during disconnect cleanup).
    pub fn unsubscribe_all(&self, session_id: &str) {
        let mounts = self.mounts.read();
        for mount in mounts.values() {
            mount.unsubscribe(session_id);
        }
    }
}

impl Default for MountRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the mount path from an RTSP URI.
///
/// `rtsp://host:8554/stream/track1` → `/stream`
/// `rtsp://host:8554/stream`        → `/stream`
/// `rtsp://host:8554/`              → `/`
/// `rtsp://host:8554`               → `/stream` (default)
/// `*`                               → `/stream` (default)
pub fn extract_mount_path(uri: &str) -> &str {
    let path = if let Some(after) = uri
        .strip_prefix("rtsp://")
        .or_else(|| uri.strip_prefix("rtsps://"))
    {
        match after.find('/') {
            Some(slash) => &after[slash..],
            None => DEFAULT_MOUNT_PATH,
        }
    } else if uri.starts_with('/') {
        uri
    } else {
        DEFAULT_MOUNT_PATH
    };

    // Strip track suffix: /stream/track1 → /stream
    if let Some(pos) = path.rfind("/track") {
        &path[..pos]
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_path_full_uri() {
        assert_eq!(
            extract_mount_path("rtsp://localhost:8554/stream"),
            "/stream"
        );
    }

    #[test]
    fn extract_path_with_track() {
        assert_eq!(
            extract_mount_path("rtsp://localhost:8554/stream/track1"),
            "/stream"
        );
    }

    #[test]
    fn extract_path_no_path() {
        assert_eq!(
            extract_mount_path("rtsp://localhost:8554"),
            DEFAULT_MOUNT_PATH
        );
    }

    #[test]
    fn extract_path_star() {
        assert_eq!(extract_mount_path("*"), DEFAULT_MOUNT_PATH);
    }

    #[test]
    fn extract_path_bare_path() {
        assert_eq!(extract_mount_path("/camera1"), "/camera1");
    }

    #[test]
    fn extract_path_with_camera_track() {
        assert_eq!(
            extract_mount_path("rtsp://10.0.0.1:8554/camera1/track1"),
            "/camera1"
        );
    }

    #[test]
    fn subscribe_unsubscribe() {
        let mount = Mount::new(
            "/test",
            Box::new(crate::media::h264::H264Packetizer::new(96, 0x1234)),
        );
        mount.subscribe("session1");
        mount.subscribe("session2");
        assert_eq!(mount.subscribed_session_ids().len(), 2);

        mount.unsubscribe("session1");
        assert_eq!(mount.subscribed_session_ids(), vec!["session2"]);
    }

    #[test]
    fn subscribe_idempotent() {
        let mount = Mount::new(
            "/test",
            Box::new(crate::media::h264::H264Packetizer::new(96, 0x1234)),
        );
        mount.subscribe("session1");
        mount.subscribe("session1");
        assert_eq!(mount.subscribed_session_ids().len(), 1);
    }

    #[test]
    fn registry_add_and_get() {
        let registry = MountRegistry::new();
        let p = Box::new(crate::media::h264::H264Packetizer::new(96, 0x1234));
        registry.add("/stream", p);

        assert!(registry.get("/stream").is_some());
        assert!(registry.get("/other").is_none());
    }

    #[test]
    fn registry_resolve_from_uri() {
        let registry = MountRegistry::new();
        let p = Box::new(crate::media::h264::H264Packetizer::new(96, 0x1234));
        registry.add("/stream", p);

        assert!(
            registry
                .resolve_from_uri("rtsp://localhost:8554/stream")
                .is_some()
        );
        assert!(
            registry
                .resolve_from_uri("rtsp://localhost:8554/stream/track1")
                .is_some()
        );
        // No default set — unknown path returns None
        assert!(
            registry
                .resolve_from_uri("rtsp://localhost:8554/other")
                .is_none()
        );
    }

    #[test]
    fn registry_resolve_fallback_to_default() {
        let registry = MountRegistry::new();
        let p = Box::new(crate::media::h264::H264Packetizer::new(96, 0x1234));
        registry.add("/stream", p);
        registry.set_default("/stream");

        // Exact match still works
        let mount = registry
            .resolve_from_uri("rtsp://localhost:8554/stream")
            .unwrap();
        assert_eq!(mount.path(), "/stream");

        // Unknown path falls back to default
        let mount = registry
            .resolve_from_uri("rtsp://localhost:8554/test")
            .unwrap();
        assert_eq!(mount.path(), "/stream");

        // Even an arbitrary path falls back
        let mount = registry
            .resolve_from_uri("rtsp://localhost:8554/anything")
            .unwrap();
        assert_eq!(mount.path(), "/stream");
    }

    #[test]
    fn registry_unsubscribe_all() {
        let registry = MountRegistry::new();
        let p1 = Box::new(crate::media::h264::H264Packetizer::new(96, 0x1234));
        let p2 = Box::new(crate::media::h264::H264Packetizer::new(96, 0x5678));
        registry.add("/stream1", p1);
        registry.add("/stream2", p2);

        registry.get("/stream1").unwrap().subscribe("sess1");
        registry.get("/stream2").unwrap().subscribe("sess1");

        registry.unsubscribe_all("sess1");

        assert!(
            registry
                .get("/stream1")
                .unwrap()
                .subscribed_session_ids()
                .is_empty()
        );
        assert!(
            registry
                .get("/stream2")
                .unwrap()
                .subscribed_session_ids()
                .is_empty()
        );
    }
}
