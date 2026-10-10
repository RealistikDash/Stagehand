//! Preview images: a static texture for the grid, and frame stepping for GIF previews on hover.

use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use std::time::SystemTime;

use gdk_pixbuf::Pixbuf;
use gdk_pixbuf::PixbufAnimation;
use gdk_pixbuf::prelude::*;
use gtk4::gdk;
use gtk4::glib::object::Cast;

const MIN_FRAME_DELAY: Duration = Duration::from_millis(20);
const DEFAULT_FRAME_DELAY: Duration = Duration::from_millis(100);

/// Decodes in a worker thread, scaled down to `max_width` so that a card's natural size stays
/// small; the result is a texture for the main thread.
pub async fn load_texture(path: PathBuf, max_width: i32) -> Option<gdk::Texture> {
    gio::spawn_blocking(move || {
        let pixbuf = Pixbuf::from_file_at_scale(&path, max_width, -1, true).ok()?;

        texture_from_pixbuf(&pixbuf).map(Cast::upcast)
    })
    .await
    .ok()
    .flatten()
}

pub fn is_animated(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
}

type PendingFrame = Rc<RefCell<Option<glib::SourceId>>>;

/// Plays a GIF into a picture until [`Animation::stop`] is called.
///
/// Every frame is scaled to `size`, the dimensions of the still texture the picture shows
/// otherwise. A picture's natural size follows its paintable, so a frame of a different size
/// would resize the card, shift the grid under the pointer and fire leave/enter in a loop.
pub struct Animation {
    pending: PendingFrame,
}

impl Animation {
    pub fn start(path: &Path, picture: &gtk4::Picture, size: (i32, i32)) -> Option<Self> {
        let animation = PixbufAnimation::from_file(path).ok()?;

        if animation.is_static_image() {
            return None;
        }

        let pending: PendingFrame = Rc::new(RefCell::new(None));
        schedule_frame(animation.iter(None), picture.clone(), size, &pending);

        Some(Self { pending })
    }

    pub fn stop(self, picture: &gtk4::Picture, still: Option<&gdk::Texture>) {
        if let Some(source) = self.pending.borrow_mut().take() {
            source.remove();
        }

        picture.set_paintable(still);
    }
}

fn schedule_frame(
    iterator: gdk_pixbuf::PixbufAnimationIter,
    picture: gtk4::Picture,
    size: (i32, i32),
    pending: &PendingFrame,
) {
    let delay = iterator
        .delay_time()
        .map_or(DEFAULT_FRAME_DELAY, |delay| delay.max(MIN_FRAME_DELAY));
    let next = Rc::clone(pending);
    let source = glib::timeout_add_local_once(delay, move || {
        iterator.advance(SystemTime::now());
        let frame = iterator.pixbuf();
        let (width, height) = size;
        let scaled = if frame.width() == width && frame.height() == height {
            Some(frame)
        } else {
            frame.scale_simple(width, height, gdk_pixbuf::InterpType::Bilinear)
        };
        picture.set_paintable(
            scaled
                .and_then(|frame| texture_from_pixbuf(&frame))
                .as_ref(),
        );
        *next.borrow_mut() = None;
        schedule_frame(iterator, picture, size, &next);
    });
    *pending.borrow_mut() = Some(source);
}

/// GTK 4.20 deprecated its pixbuf bridge; the frame is copied into a memory texture instead.
fn texture_from_pixbuf(pixbuf: &Pixbuf) -> Option<gdk::MemoryTexture> {
    let format = if pixbuf.has_alpha() {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    let bytes = pixbuf.read_pixel_bytes();
    let stride = usize::try_from(pixbuf.rowstride()).ok()?;

    Some(gdk::MemoryTexture::new(
        pixbuf.width(),
        pixbuf.height(),
        format,
        &bytes,
        stride,
    ))
}
