use stagehand_core::config::PropertyValues;
use stagehand_core::model::Volume;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("GStreamer: {0}")]
    GStreamer(#[from] gstreamer::glib::Error),
    #[error("GStreamer: {0}")]
    GStreamerBool(#[from] gstreamer::glib::BoolError),
    #[error("GStreamer state change failed: {0}")]
    StateChange(#[from] gstreamer::StateChangeError),
    #[error("{0} is not a valid path")]
    InvalidPath(String),
}

/// A wallpaper widget. Implemented once per wallpaper kind.
pub trait Content {
    fn widget(&self) -> gtk4::Widget;
    fn pause(&self);
    fn resume(&self);
    fn set_audio(&self, muted: bool, volume: Volume);
    fn set_properties(&self, values: &PropertyValues);
}
