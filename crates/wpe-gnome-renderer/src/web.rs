use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk4::prelude::*;
use webkit6::prelude::*;
use wpe_gnome_core::config::PropertyValues;
use wpe_gnome_core::model::PropertyValue;
use wpe_gnome_core::model::Volume;
use wpe_gnome_core::protocol::RendererArguments;

use crate::content::Content;
use crate::content::ContentError;

// Installed before any page script runs. Wallpaper Engine pages register listeners on these
// globals; the audio listener receives nothing because no audio capture is wired up.
const PRELUDE_SCRIPT: &str = r"
(() => {
    window.wallpaperRegisterAudioListener = listener => {
        window.__wpeGnomeAudioListener = listener;
    };
    window.wallpaperRequestRandomFileForProperty = () => {};
    window.__wpeGnomeApply = (user, general) => {
        const listener = window.wallpaperPropertyListener;
        if (!listener) return;
        if (general && typeof listener.applyGeneralProperties === 'function')
            listener.applyGeneralProperties(general);
        if (user && typeof listener.applyUserProperties === 'function')
            listener.applyUserProperties(user);
    };
    window.__wpeGnomeSetPaused = paused => {
        const listener = window.wallpaperPropertyListener;
        if (listener && typeof listener.setPaused === 'function')
            listener.setPaused(paused);
    };
})();
";

pub struct WebContent {
    view: webkit6::WebView,
    properties: Rc<RefCell<PropertyValues>>,
    fps: u32,
}

impl WebContent {
    pub fn new(index: &Path, arguments: &RendererArguments) -> Result<Self, ContentError> {
        let settings = webkit6::Settings::builder()
            .enable_webgl(true)
            .enable_javascript(true)
            .allow_file_access_from_file_urls(true)
            .allow_universal_access_from_file_urls(true)
            .media_playback_requires_user_gesture(false)
            .enable_media_stream(false)
            .enable_smooth_scrolling(false)
            .hardware_acceleration_policy(webkit6::HardwareAccelerationPolicy::Always)
            .build();
        let manager = webkit6::UserContentManager::new();
        manager.add_script(&webkit6::UserScript::new(
            PRELUDE_SCRIPT,
            webkit6::UserContentInjectedFrames::TopFrame,
            webkit6::UserScriptInjectionTime::Start,
            &[],
            &[],
        ));
        let view = webkit6::WebView::builder()
            .settings(&settings)
            .user_content_manager(&manager)
            .hexpand(true)
            .vexpand(true)
            .build();
        view.set_background_color(&gtk4::gdk::RGBA::BLACK);
        view.set_is_muted(arguments.muted);

        let properties = Rc::new(RefCell::new(arguments.properties.clone()));
        let fps = arguments.fps.get();

        view.connect_load_changed({
            let properties = Rc::clone(&properties);

            move |view, event| {
                if event == webkit6::LoadEvent::Finished {
                    tracing::info!("Web wallpaper loaded; pushing properties.");
                    push_properties(view, &properties.borrow(), fps);
                }
            }
        });

        let uri = gtk4::glib::filename_to_uri(index, None)
            .map_err(|_| ContentError::InvalidPath(index.display().to_string()))?;
        view.load_uri(&uri);
        tracing::info!(path = %index.display(), "Loading web wallpaper.");

        Ok(Self { view, properties, fps })
    }
}

fn push_properties(view: &webkit6::WebView, values: &PropertyValues, fps: u32) {
    let user = user_properties_json(values);
    let script = format!("window.__wpeGnomeApply({user}, {{\"fps\":{fps}}});");
    run(view, &script);
}

fn run(view: &webkit6::WebView, script: &str) {
    view.evaluate_javascript(script, None, None, None::<&gio::Cancellable>, |result| {
        if let Err(error) = result {
            tracing::warn!(error = %error, "Wallpaper script call failed.");
        }
    });
}

/// The `{ name: { value } }` shape `applyUserProperties` expects. Colours are `"r g b"` strings.
fn user_properties_json(values: &PropertyValues) -> String {
    let object: serde_json::Map<String, serde_json::Value> = values
        .iter()
        .map(|(key, value)| {
            let json = match value {
                PropertyValue::Bool(flag) => serde_json::Value::Bool(*flag),
                PropertyValue::Number(number) => serde_json::json!(number),
                PropertyValue::Colour(colour) => {
                    serde_json::Value::String(colour.to_wallpaper_engine())
                }
                PropertyValue::Text(text) => serde_json::Value::String(text.clone()),
            };

            (key.clone(), serde_json::json!({ "value": json }))
        })
        .collect();

    serde_json::Value::Object(object).to_string()
}

impl Content for WebContent {
    fn widget(&self) -> gtk4::Widget {
        self.view.clone().upcast()
    }

    fn pause(&self) {
        run(&self.view, "window.__wpeGnomeSetPaused(true);");
    }

    fn resume(&self) {
        run(&self.view, "window.__wpeGnomeSetPaused(false);");
    }

    fn set_audio(&self, muted: bool, _volume: Volume) {
        self.view.set_is_muted(muted);
    }

    fn set_properties(&self, values: &PropertyValues) {
        self.properties
            .borrow_mut()
            .extend(values.iter().map(|(key, value)| (key.clone(), value.clone())));
        push_properties(&self.view, &self.properties.borrow(), self.fps);
    }
}
