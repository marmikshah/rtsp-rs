use std::sync::{LazyLock, Mutex};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;

use rtsp::Server;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtspserversink",
        gst::DebugColorFlags::empty(),
        Some("RTSP Server Sink"),
    )
});

const DEFAULT_ADDRESS: &str = "0.0.0.0";
const DEFAULT_PORT: u32 = 8554;

#[derive(Debug, Clone)]
struct Settings {
    address: String,
    port: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            address: DEFAULT_ADDRESS.to_string(),
            port: DEFAULT_PORT,
        }
    }
}

struct State {
    server: Server,
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
    type ParentType = gst_base::BaseSink;
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
            ]
        })
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "address" => {
                settings.address = value.get::<String>().expect("type checked upstream");
            }
            "port" => {
                settings.port = value.get::<u32>().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "address" => settings.address.to_value(),
            "port" => settings.port.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtspServerSink {}

impl ElementImpl for RtspServerSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: std::sync::OnceLock<gst::subclass::ElementMetadata> =
            std::sync::OnceLock::new();
        Some(ELEMENT_METADATA.get_or_init(|| {
            gst::subclass::ElementMetadata::new(
                "RTSP Server Sink",
                "Sink/Network",
                "Publishes incoming encoded video as an RTSP stream",
                "Author <marmikshah@icloud.com>",
            )
        }))
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: std::sync::OnceLock<Vec<gst::PadTemplate>> =
            std::sync::OnceLock::new();
        PAD_TEMPLATES.get_or_init(|| {
            let caps = gst::Caps::builder("video/x-h264")
                .field("stream-format", "byte-stream")
                .build();

            vec![
                gst::PadTemplate::new(
                    "sink",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Always,
                    &caps,
                )
                .unwrap(),
            ]
        })
    }
}

impl BaseSinkImpl for RtspServerSink {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap().clone();
        let bind_addr = format!("{}:{}", settings.address, settings.port);

        let mut server = Server::new(&bind_addr);

        server.start().map_err(|e| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                ["Failed to start RTSP server: {}", e]
            )
        })?;

        *self.state.lock().unwrap() = Some(State { server });

        gst::info!(CAT, imp = self, "RTSP server started on {}", bind_addr);

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        if let Some(mut state) = self.state.lock().unwrap().take() {
            state.server.stop();
            gst::info!(CAT, imp = self, "RTSP server stopped");
        }
        Ok(())
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Failed to map buffer readable");
            gst::FlowError::Error
        })?;

        let ts_increment = buffer
            .duration()
            .map(|d| ((d.nseconds() * 90000 + 500_000_000) / 1_000_000_000) as u32)
            .unwrap_or(3000);

        let state_guard = self.state.lock().unwrap();
        let state = state_guard.as_ref().ok_or_else(|| {
            gst::error!(CAT, imp = self, "Element not started");
            gst::FlowError::Error
        })?;

        if let Err(e) = state.server.send_frame(map.as_slice(), ts_increment) {
            gst::warning!(CAT, imp = self, "send_frame failed: {}", e);
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
