use std::collections::HashMap;

/// The object the GNOME Shell extension exports on the Shell's own bus name.
#[zbus::proxy(
    interface = "io.github.realistik.WpeGnome.Shell",
    default_service = "org.gnome.Shell",
    default_path = "/io/github/realistik/WpeGnome/Shell"
)]
pub trait Shell {
    fn pin(&self, pid: u32, connectors: &[&str]) -> zbus::Result<()>;
    fn unpin(&self, pid: u32) -> zbus::Result<()>;
    fn set_visible(&self, pid: u32, visible: bool) -> zbus::Result<()>;
    fn set_dim(&self, pid: u32, level: f64) -> zbus::Result<()>;

    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn occlusion_states(&self) -> zbus::Result<HashMap<String, String>>;

    #[zbus(signal)]
    fn window_pinned(&self, pid: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn window_lost(&self, pid: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn occlusion_changed(&self, connector: &str, state: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn ready(&self) -> zbus::Result<()>;
}
