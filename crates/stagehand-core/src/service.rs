//! The daemon's state machine: one actor that owns configuration, library, monitors and the
//! renderer slots, and processes [`Event`]s from a single channel.

mod context;
mod events;
mod plan;
mod runner;

pub use context::Context;
pub use events::Event;
pub use events::Notification;
pub use events::Occlusion;
pub use events::Reply;
pub use events::Request;
pub use events::ServiceError;
pub use plan::DesiredSlot;
pub use plan::desired_slots;
pub use plan::merged_properties;
pub use runner::Service;
