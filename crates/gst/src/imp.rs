use std::sync::{LazyLock, Mutex};

use gstreamer::glib;
use gstreamer::prelude::*;
use gstreamer::subclass::prelude::*;
use gstreamer_base::subclass::prelude::*;

use rtsp::Server;

static CAT: LazyLock<gstreamer::DebugCategory> = LazyLock::new(|| {
    gstreamer::DebugCategory::new(
        "rtspserversink",
        gstreamer::DebugColorFlags::empty(),
        Some("RTSP Server Sink"),
    )
});

const DEFAULT_ADDRESS: &str = "0.0.0.0";
const DEFAULT_PORT: u32 = 8554;
const DEFAULT_MOUNT_PATH: &str = "/stream";

#[derive(Debug, Clone)]
struct Settings {
    address: String,
    port: u32,
    mount_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            address: DEFAULT_ADDRESS.to_string(),
            port: DEFAULT_PORT,
            mount_path: DEFAULT_MOUNT_PATH.to_string(),
        }
    }
}

struct State {
    server: Server,
    mount_path: String,
}

pub struct RtspServerSink {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

impl Default for RtspServerSink {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(None),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtspServerSink {
    const NAME: &'static str = "GstRtspServerSink";
    type Type = super::RtspServerSink;
    type ParentType = gstreamer_base::BaseSink;
}

impl ObjectImpl for RtspServerSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: std::sync::OnceLock<Vec<glib::ParamSpec>> = std::sync::OnceLock::new();
        PROPERTIES.get_or_init(|| {
            vec![
                glib::ParamSpecString::builder("address")
                    .nick("Bind Address")
                    .blurb("Address to bind the RTSP server to")
                    .default_value(Some(DEFAULT_ADDRESS))
                    .build(),
                glib::ParamSpecUInt::builder("port")
                    .nick("Port")
                    .blurb("Port for the RTSP server")
                    .minimum(1)
                    .maximum(65535)
                    .default_value(DEFAULT_PORT)
                    .build(),
                glib::ParamSpecString::builder("mount-path")
                    .nick("Mount Path")
                    .blurb("RTSP stream path (e.g. /stream or /cam1)")
                    .default_value(Some(DEFAULT_MOUNT_PATH))
                    .build(),
            ]
        })
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "address" => {
                if let Ok(s) = value.get::<String>() {
                    settings.address = s;
                }
            }
            "port" => {
                if let Ok(p) = value.get::<u32>() {
                    settings.port = p;
                }
            }
            "mount-path" => {
                if let Ok(s) = value.get::<String>() {
                    settings.mount_path = s;
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "address" => settings.address.to_value(),
            "port" => settings.port.to_value(),
            "mount-path" => settings.mount_path.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtspServerSink {}

impl ElementImpl for RtspServerSink {
    fn metadata() -> Option<&'static gstreamer::subclass::ElementMetadata> {
        static ELEMENT_METADATA: std::sync::OnceLock<gstreamer::subclass::ElementMetadata> =
            std::sync::OnceLock::new();
        Some(ELEMENT_METADATA.get_or_init(|| {
            gstreamer::subclass::ElementMetadata::new(
                "RTSP Server Sink",
                "Sink/Network",
                "Publishes incoming encoded video as an RTSP stream",
                "Author <marmikshah@icloud.com>",
            )
        }))
    }

    fn pad_templates() -> &'static [gstreamer::PadTemplate] {
        static PAD_TEMPLATES: std::sync::OnceLock<Vec<gstreamer::PadTemplate>> =
            std::sync::OnceLock::new();
        PAD_TEMPLATES.get_or_init(|| {
            let caps = gstreamer::Caps::builder("video/x-h264")
                .field("stream-format", "byte-stream")
                .build();

            vec![
                gstreamer::PadTemplate::new(
                    "sink",
                    gstreamer::PadDirection::Sink,
                    gstreamer::PadPresence::Always,
                    &caps,
                )
                .unwrap(),
            ]
        })
    }
}

impl BaseSinkImpl for RtspServerSink {
    fn start(&self) -> Result<(), gstreamer::ErrorMessage> {
        let settings = self.settings.lock().unwrap().clone();
        let bind_addr = format!("{}:{}", settings.address, settings.port);

        let mut server = Server::new_with_mount_path(&bind_addr, &settings.mount_path);

        server.start().map_err(|e| {
            gstreamer::error_msg!(
                gstreamer::ResourceError::OpenWrite,
                ["Failed to start RTSP server: {}", e]
            )
        })?;

        let mount_path = settings.mount_path.clone();
        *self.state.lock().unwrap() = Some(State {
            server,
            mount_path: mount_path.clone(),
        });

        gstreamer::info!(
            CAT,
            imp = self,
            "RTSP server started on {} mount {}",
            bind_addr,
            mount_path
        );

        Ok(())
    }

    fn stop(&self) -> Result<(), gstreamer::ErrorMessage> {
        if let Some(mut state) = self.state.lock().unwrap().take() {
            state.server.stop();
            gstreamer::info!(CAT, imp = self, "RTSP server stopped");
        }
        Ok(())
    }

    fn render(&self, buffer: &gstreamer::Buffer) -> Result<gstreamer::FlowSuccess, gstreamer::FlowError> {
        let map = buffer.map_readable().map_err(|_| {
            gstreamer::error!(CAT, imp = self, "Failed to map buffer readable");
            gstreamer::FlowError::Error
        })?;

        let ts_increment = buffer
            .duration()
            .map(|d| ((d.nseconds() * 90000 + 500_000_000) / 1_000_000_000) as u32)
            .unwrap_or(3000);

        let state_guard = self.state.lock().unwrap();
        let state = state_guard.as_ref().ok_or_else(|| {
            gstreamer::error!(CAT, imp = self, "Element not started");
            gstreamer::FlowError::Error
        })?;

        if let Err(e) = state
            .server
            .send_frame_to(&state.mount_path, map.as_slice(), ts_increment)
        {
            gstreamer::warning!(CAT, imp = self, "send_frame failed: {}", e);
        }

        Ok(gstreamer::FlowSuccess::Ok)
    }
}
