//! The two D-Bus interfaces of stagehand. Complex values cross the bus as JSON strings of the
//! core models, so there is one serialisation format for the configuration file and the wire.

mod daemon;
pub mod names;
mod shell;

pub use daemon::DaemonProxy;
pub use daemon::IpcError;
pub use daemon::decode;
pub use daemon::encode;
pub use shell::ShellProxy;
