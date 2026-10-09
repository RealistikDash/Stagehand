pub const DAEMON_BUS_NAME: &str = "io.github.realistikdash.Stagehand.Daemon";
pub const DAEMON_OBJECT_PATH: &str = "/io/github/realistikdash/Stagehand";
pub const DAEMON_INTERFACE: &str = "io.github.realistikdash.Stagehand.Daemon";

pub const SHELL_BUS_NAME: &str = "org.gnome.Shell";
pub const SHELL_OBJECT_PATH: &str = "/io/github/realistikdash/Stagehand/Shell";
pub const SHELL_INTERFACE: &str = "io.github.realistikdash.Stagehand.Shell";
/// Bumped whenever the extension interface changes incompatibly; the daemon refuses older ones.
pub const SHELL_INTERFACE_VERSION: u32 = 1;

pub const EXTENSION_UUID: &str = "stagehand@realistikdash.github.io";
pub const RENDERER_APP_ID: &str = "io.github.realistikdash.Stagehand.Renderer";
pub const UI_APP_ID: &str = "io.github.realistikdash.Stagehand";

pub const OCCLUSION_NONE: &str = "none";
pub const OCCLUSION_MAXIMISED: &str = "maximised";
pub const OCCLUSION_FULLSCREEN: &str = "fullscreen";
