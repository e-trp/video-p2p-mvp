#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxCaptureBackend {
    PortalPipeWire,
    X11Fallback,
}

#[derive(Debug, Clone)]
pub struct LinuxCaptureBlueprint {
    pub preferred_backend: LinuxCaptureBackend,
    pub notes: Vec<&'static str>,
}

pub fn blueprint() -> LinuxCaptureBlueprint {
    LinuxCaptureBlueprint {
        preferred_backend: LinuxCaptureBackend::PortalPipeWire,
        notes: vec![
            "use XDG Desktop Portal ScreenCast for Wayland source selection",
            "consume resulting PipeWire stream for video and available audio",
            "fallback to X11 capture only where Wayland path is unavailable",
            "keep GUI flow aligned with portal-driven user permission model",
        ],
    }
}
