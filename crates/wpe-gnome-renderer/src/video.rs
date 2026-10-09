use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use gstreamer as gst;
use gstreamer::prelude::*;
use gtk4::prelude::*;
use wpe_gnome_core::config::PropertyValues;
use wpe_gnome_core::model::Volume;
use wpe_gnome_core::protocol::RendererArguments;

use crate::content::Content;
use crate::content::ContentError;
use crate::window;

/// A looping video through `playbin3` and `gtk4paintablesink`. Looping uses segment seeks, so the
/// pipeline never reaches end-of-stream and the loop point has no gap.
pub struct VideoContent {
    pipeline: gst::Element,
    picture: gtk4::Picture,
    // Kept alive for the lifetime of the pipeline; dropping it removes the watch.
    _bus_watch: gst::bus::BusWatchGuard,
}

impl VideoContent {
    pub fn new(file: &Path, arguments: &RendererArguments) -> Result<Self, ContentError> {
        gst::init()?;

        let sink = gst::ElementFactory::make("gtk4paintablesink").build()?;
        let paintable = sink.property::<gtk4::gdk::Paintable>("paintable");
        let picture = gtk4::Picture::for_paintable(&paintable);
        picture.set_content_fit(window::content_fit(arguments.scaling));
        picture.set_hexpand(true);
        picture.set_vexpand(true);

        let rate_limit = gst::ElementFactory::make("videorate")
            .property("drop-only", true)
            .property("max-rate", i32::try_from(arguments.fps.get()).unwrap_or(i32::MAX))
            .build()?;
        let uri = gst::glib::filename_to_uri(file, None)
            .map_err(|_| ContentError::InvalidPath(file.display().to_string()))?;
        let pipeline = gst::ElementFactory::make("playbin3")
            .property("uri", uri.as_str())
            .property("video-sink", &sink)
            .property("video-filter", &rate_limit)
            .property("volume", arguments.volume.fraction())
            .property("mute", arguments.muted)
            .build()?;
        let bus = pipeline.bus().ok_or_else(|| gst::glib::bool_error!("playbin has no bus"))?;
        let looped_once = Rc::new(Cell::new(false));
        let bus_watch = {
            let pipeline = pipeline.clone();

            bus.add_watch_local(move |_, message| {
                handle_message(&pipeline, message, &looped_once);

                gst::glib::ControlFlow::Continue
            })?
        };

        pipeline.set_state(gst::State::Playing)?;
        tracing::info!(path = %file.display(), "Started video pipeline.");

        Ok(Self { pipeline, picture, _bus_watch: bus_watch })
    }
}

fn handle_message(pipeline: &gst::Element, message: &gst::Message, looped_once: &Cell<bool>) {
    use gst::MessageView;

    match message.view() {
        MessageView::AsyncDone(_) if !looped_once.get() => {
            // The first segment seek switches the pipeline into segment mode; later loops are
            // non-flushing seeks triggered by SegmentDone.
            looped_once.set(true);
            seek_to_start(pipeline, gst::SeekFlags::FLUSH | gst::SeekFlags::SEGMENT);
        }
        MessageView::SegmentDone(_) => {
            seek_to_start(pipeline, gst::SeekFlags::SEGMENT);
        }
        MessageView::Eos(_) => {
            seek_to_start(pipeline, gst::SeekFlags::FLUSH | gst::SeekFlags::SEGMENT);
        }
        MessageView::Error(error) => {
            tracing::error!(
                source = ?error.src().map(|source| source.path_string()),
                error = %error.error(),
                debug = ?error.debug(),
                "Video pipeline error."
            );
        }
        MessageView::Warning(warning) => {
            tracing::warn!(error = %warning.error(), "Video pipeline warning.");
        }
        _ => {}
    }
}

fn seek_to_start(pipeline: &gst::Element, flags: gst::SeekFlags) {
    let result = pipeline.seek(
        1.0,
        flags | gst::SeekFlags::ACCURATE,
        gst::SeekType::Set,
        gst::ClockTime::ZERO,
        gst::SeekType::None,
        gst::ClockTime::NONE,
    );

    if let Err(error) = result {
        tracing::warn!(error = %error, "Loop seek failed.");
    }
}

impl Content for VideoContent {
    fn widget(&self) -> gtk4::Widget {
        self.picture.clone().upcast()
    }

    fn pause(&self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Paused) {
            tracing::warn!(error = %error, "Could not pause video.");
        }
    }

    fn resume(&self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Playing) {
            tracing::warn!(error = %error, "Could not resume video.");
        }
    }

    fn set_audio(&self, muted: bool, volume: Volume) {
        self.pipeline.set_property("mute", muted);
        self.pipeline.set_property("volume", volume.fraction());
    }

    fn set_properties(&self, _values: &PropertyValues) {}
}
