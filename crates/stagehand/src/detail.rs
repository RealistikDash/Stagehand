//! The detail pane: one wallpaper, its apply targets and its property editor.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gtk4::gdk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use stagehand_core::config::PropertyValues;
use stagehand_core::model::Colour;
use stagehand_core::model::LibraryItem;
use stagehand_core::model::MonitorInfo;
use stagehand_core::model::MonitorKey;
use stagehand_core::model::Property;
use stagehand_core::model::PropertyKind;
use stagehand_core::model::PropertyValue;
use stagehand_core::model::Status;
use stagehand_core::model::Support;
use stagehand_core::model::UnsupportedReason;
use stagehand_core::model::Wallpaper;
use stagehand_core::model::WallpaperChoice;
use stagehand_core::model::WallpaperId;

use crate::preview;

// Sliders fire continuously; scenes restart on every change, so changes are coalesced.
const PROPERTY_DEBOUNCE: Duration = Duration::from_millis(700);
const DETAIL_PREVIEW_WIDTH: i32 = 420;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyTarget {
    Monitor(MonitorKey),
    Span,
}

type ApplyHandler = Box<dyn Fn(ApplyTarget, WallpaperChoice)>;
type PropertiesHandler = Box<dyn Fn(WallpaperId, PropertyValues)>;
type CloseHandler = Box<dyn Fn()>;

/// The editor's live values, written back to the daemon after a short delay.
struct Editor {
    wallpaper: WallpaperId,
    defaults: BTreeMap<String, PropertyValue>,
    values: BTreeMap<String, PropertyValue>,
    pending: Option<glib::SourceId>,
}

pub struct DetailPane {
    pub widget: gtk4::ScrolledWindow,
    picture: gtk4::Picture,
    title: gtk4::Label,
    subtitle: gtk4::Label,
    description: gtk4::Label,
    link: gtk4::LinkButton,
    unsupported: adw::Banner,
    targets: adw::PreferencesGroup,
    properties: adw::PreferencesGroup,
    reset: gtk4::Button,
    editor: Rc<RefCell<Option<Editor>>>,
    on_apply: Rc<RefCell<Option<ApplyHandler>>>,
    on_properties: Rc<RefCell<Option<PropertiesHandler>>>,
    on_close: Rc<RefCell<Option<CloseHandler>>>,
}

impl DetailPane {
    pub fn new() -> Self {
        let picture = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .height_request(180)
            .overflow(gtk4::Overflow::Hidden)
            .build();
        picture.add_css_class("card");
        let title = gtk4::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .build();
        title.add_css_class("title-2");
        let subtitle = gtk4::Label::builder().xalign(0.0).wrap(true).build();
        subtitle.add_css_class("dim-label");
        let description = gtk4::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .lines(6)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .selectable(true)
            .build();
        description.add_css_class("body");
        let link = gtk4::LinkButton::builder()
            .label("Open in Steam Workshop")
            .halign(gtk4::Align::Start)
            .build();
        let unsupported = adw::Banner::builder().revealed(false).build();
        let targets = adw::PreferencesGroup::builder().title("Apply").build();
        let reset = gtk4::Button::builder()
            .label("Reset")
            .valign(gtk4::Align::Center)
            .tooltip_text("Restore the wallpaper's default properties")
            .build();
        reset.add_css_class("flat");
        let properties = adw::PreferencesGroup::builder()
            .title("Properties")
            .header_suffix(&reset)
            .build();
        let close = gtk4::Button::builder()
            .icon_name("window-close-symbolic")
            .halign(gtk4::Align::End)
            .tooltip_text("Close")
            .build();
        close.add_css_class("flat");
        close.add_css_class("circular");

        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .margin_start(16)
            .margin_end(16)
            .margin_top(12)
            .margin_bottom(16)
            .build();
        content.append(&close);
        content.append(&picture);
        content.append(&title);
        content.append(&subtitle);
        content.append(&unsupported);
        content.append(&description);
        content.append(&link);
        content.append(&targets);
        content.append(&properties);
        let widget = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&content)
            .build();

        let pane = Self {
            widget,
            picture,
            title,
            subtitle,
            description,
            link,
            unsupported,
            targets,
            properties,
            reset,
            editor: Rc::new(RefCell::new(None)),
            on_apply: Rc::new(RefCell::new(None)),
            on_properties: Rc::new(RefCell::new(None)),
            on_close: Rc::new(RefCell::new(None)),
        };
        let on_close = Rc::clone(&pane.on_close);
        close.connect_clicked(move |_| {
            if let Some(handler) = on_close.borrow().as_ref() {
                handler();
            }
        });

        pane
    }

    pub fn set_on_apply(&self, handler: impl Fn(ApplyTarget, WallpaperChoice) + 'static) {
        *self.on_apply.borrow_mut() = Some(Box::new(handler));
    }

    pub fn set_on_properties(&self, handler: impl Fn(WallpaperId, PropertyValues) + 'static) {
        *self.on_properties.borrow_mut() = Some(Box::new(handler));
    }

    pub fn set_on_close(&self, handler: impl Fn() + 'static) {
        *self.on_close.borrow_mut() = Some(Box::new(handler));
    }

    pub fn show(
        &self,
        item: &LibraryItem,
        monitors: &[MonitorInfo],
        status: &Status,
        saved: Option<&PropertyValues>,
    ) {
        match item {
            LibraryItem::Wallpaper(wallpaper) => {
                self.show_wallpaper(wallpaper, monitors, status, saved);
            }
            LibraryItem::Broken(broken) => {
                self.title.set_text(item.title());
                self.subtitle.set_text("Broken project");
                self.description.set_text(&broken.error);
                self.link.set_visible(false);
                self.unsupported
                    .set_title("This project.json could not be read.");
                self.unsupported.set_revealed(true);
                self.picture.set_paintable(None::<&gdk::Paintable>);
                clear_group(&self.targets);
                clear_group(&self.properties);
                self.properties.set_visible(false);
                *self.editor.borrow_mut() = None;
            }
        }
    }

    fn show_wallpaper(
        &self,
        wallpaper: &Wallpaper,
        monitors: &[MonitorInfo],
        status: &Status,
        saved: Option<&PropertyValues>,
    ) {
        let same_wallpaper = self
            .editor
            .borrow()
            .as_ref()
            .is_some_and(|editor| editor.wallpaper == wallpaper.id);

        self.title.set_text(&wallpaper.title);
        self.subtitle.set_text(&subtitle_for(wallpaper));
        self.description.set_text(&wallpaper.description);
        self.description
            .set_visible(!wallpaper.description.trim().is_empty());

        match &wallpaper.workshop_url {
            Some(url) => {
                self.link.set_uri(url);
                self.link.set_visible(true);
            }
            None => self.link.set_visible(false),
        }

        match &wallpaper.support {
            Support::Supported => self.unsupported.set_revealed(false),
            Support::Unsupported(reason) => {
                self.unsupported.set_title(&unsupported_text(reason));
                self.unsupported.set_revealed(true);
            }
        }

        if !same_wallpaper {
            self.picture.set_paintable(None::<&gdk::Paintable>);

            if let Some(preview) = wallpaper.preview.clone() {
                let picture = self.picture.clone();

                glib::spawn_future_local(async move {
                    if let Some(texture) =
                        preview::load_texture(preview, DETAIL_PREVIEW_WIDTH).await
                    {
                        picture.set_paintable(Some(&texture));
                    }
                });
            }
        }

        self.fill_targets(wallpaper, monitors, status);

        if !same_wallpaper {
            self.fill_properties(wallpaper, saved);
        }
    }

    fn fill_targets(&self, wallpaper: &Wallpaper, monitors: &[MonitorInfo], status: &Status) {
        clear_group(&self.targets);
        let supported = wallpaper.support == Support::Supported;
        self.targets.set_sensitive(supported);

        for monitor in monitors {
            let current = status
                .slots
                .iter()
                .find(|slot| slot.connectors.contains(&monitor.identity.connector))
                .and_then(|slot| slot.wallpaper.as_ref());
            let row = adw::ActionRow::builder()
                .title(format!(
                    "{} ({})",
                    monitor.identity.product, monitor.identity.connector
                ))
                .subtitle(format!(
                    "{}×{}",
                    monitor.logical.width, monitor.logical.height
                ))
                .build();
            let button = apply_button(current == Some(&wallpaper.id));
            let target = ApplyTarget::Monitor(monitor.identity.key());
            self.connect_apply(&button, target, wallpaper.id.clone());
            row.add_suffix(&button);
            self.targets.add(&row);
        }

        if monitors.len() > 1 {
            let row = adw::ActionRow::builder()
                .title("All monitors")
                .subtitle("One wallpaper spanning every monitor")
                .build();
            let spanning = status.slots.iter().any(|slot| {
                slot.connectors.len() > 1 && slot.wallpaper.as_ref() == Some(&wallpaper.id)
            });
            let button = apply_button(spanning);
            self.connect_apply(&button, ApplyTarget::Span, wallpaper.id.clone());
            row.add_suffix(&button);
            self.targets.add(&row);
        }
    }

    fn connect_apply(&self, button: &gtk4::Button, target: ApplyTarget, id: WallpaperId) {
        let on_apply = Rc::clone(&self.on_apply);

        button.connect_clicked(move |_| {
            if let Some(handler) = on_apply.borrow().as_ref() {
                handler(
                    target.clone(),
                    WallpaperChoice {
                        wallpaper: id.clone(),
                        scaling: None,
                    },
                );
            }
        });
    }

    fn fill_properties(&self, wallpaper: &Wallpaper, saved: Option<&PropertyValues>) {
        clear_group(&self.properties);
        let editable: Vec<&Property> = wallpaper
            .properties
            .iter()
            .filter(|property| property.is_editable() || property.kind == PropertyKind::Label)
            .collect();
        self.properties.set_visible(!editable.is_empty());

        let defaults: BTreeMap<String, PropertyValue> = wallpaper
            .properties
            .iter()
            .filter_map(|property| {
                property
                    .default
                    .clone()
                    .map(|value| (property.key.clone(), value))
            })
            .collect();
        let mut values = defaults.clone();

        if let Some(saved) = saved {
            values.extend(
                saved
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
        }

        *self.editor.borrow_mut() = Some(Editor {
            wallpaper: wallpaper.id.clone(),
            defaults,
            values: values.clone(),
            pending: None,
        });

        for property in editable {
            let current = values.get(&property.key).cloned();
            let row = self.build_row(property, current);
            self.properties.add(&row);
        }

        let editor = Rc::clone(&self.editor);
        let on_properties = Rc::clone(&self.on_properties);
        let id = wallpaper.id.clone();
        self.reset.connect_clicked(move |_| {
            {
                let mut editor = editor.borrow_mut();

                if let Some(editor) = editor.as_mut() {
                    editor.values = editor.defaults.clone();
                }
            }

            if let Some(handler) = on_properties.borrow().as_ref() {
                handler(id.clone(), PropertyValues::new());
            }
        });
    }

    fn build_row(&self, property: &Property, current: Option<PropertyValue>) -> gtk4::Widget {
        let label = humanise(&property.label);
        let key = property.key.clone();

        match &property.kind {
            PropertyKind::Bool => {
                let row = adw::SwitchRow::builder().title(&label).build();
                row.set_active(matches!(current, Some(PropertyValue::Bool(true))));
                let update = self.updater(key);
                row.connect_active_notify(move |row| update(PropertyValue::Bool(row.is_active())));

                row.upcast()
            }
            PropertyKind::Slider {
                min,
                max,
                step,
                precision,
            } => {
                let step = step.filter(|step| *step > 0.0).unwrap_or(1.0);
                let adjustment = gtk4::Adjustment::new(0.0, *min, *max, step, step * 10.0, 0.0);
                let row = adw::SpinRow::builder()
                    .title(&label)
                    .adjustment(&adjustment)
                    .digits(u32::from(precision.unwrap_or(0)))
                    .build();

                if let Some(PropertyValue::Number(value)) = current {
                    row.set_value(value);
                }

                let update = self.updater(key);
                row.connect_value_notify(move |row| update(PropertyValue::Number(row.value())));

                row.upcast()
            }
            PropertyKind::Colour => {
                let row = adw::ActionRow::builder().title(&label).build();
                let dialog = gtk4::ColorDialog::builder().with_alpha(false).build();
                let button = gtk4::ColorDialogButton::builder()
                    .dialog(&dialog)
                    .valign(gtk4::Align::Center)
                    .build();

                if let Some(PropertyValue::Colour(colour)) = current {
                    button.set_rgba(&gdk::RGBA::new(colour.red, colour.green, colour.blue, 1.0));
                }

                let update = self.updater(key);
                button.connect_rgba_notify(move |button| {
                    let rgba = button.rgba();

                    update(PropertyValue::Colour(Colour {
                        red: rgba.red(),
                        green: rgba.green(),
                        blue: rgba.blue(),
                    }));
                });
                row.add_suffix(&button);

                row.upcast()
            }
            PropertyKind::Combo { options } => {
                let labels: Vec<String> = options
                    .iter()
                    .map(|option| humanise(&option.label))
                    .collect();
                let model =
                    gtk4::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
                let row = adw::ComboRow::builder().title(&label).model(&model).build();
                let selected = options
                    .iter()
                    .position(|option| Some(&option.value) == current.as_ref())
                    .unwrap_or(0);
                row.set_selected(u32::try_from(selected).unwrap_or(0));
                let values: Vec<PropertyValue> =
                    options.iter().map(|option| option.value.clone()).collect();
                let update = self.updater(key);
                row.connect_selected_notify(move |row| {
                    if let Some(value) = values.get(usize::try_from(row.selected()).unwrap_or(0)) {
                        update(value.clone());
                    }
                });

                row.upcast()
            }
            PropertyKind::TextInput => {
                let row = adw::EntryRow::builder().title(&label).build();

                if let Some(PropertyValue::Text(text)) = current {
                    row.set_text(&text);
                }

                let update = self.updater(key);
                row.connect_apply(move |row| update(PropertyValue::Text(row.text().to_string())));
                row.set_show_apply_button(true);

                row.upcast()
            }
            PropertyKind::Label => {
                let row = adw::ActionRow::builder().title(&label).build();
                row.add_css_class("property");

                row.upcast()
            }
            PropertyKind::File
            | PropertyKind::Directory
            | PropertyKind::SceneTexture
            | PropertyKind::Unsupported { .. } => {
                let row = adw::ActionRow::builder()
                    .title(&label)
                    .subtitle("Not supported")
                    .sensitive(false)
                    .build();

                row.upcast()
            }
        }
    }

    /// Records a change and schedules the write-back.
    fn updater(&self, key: String) -> impl Fn(PropertyValue) + 'static {
        let editor = Rc::clone(&self.editor);
        let on_properties = Rc::clone(&self.on_properties);

        move |value| {
            let mut guard = editor.borrow_mut();
            let Some(state) = guard.as_mut() else {
                return;
            };

            if state.values.get(&key) == Some(&value) {
                return;
            }

            state.values.insert(key.clone(), value);

            if let Some(pending) = state.pending.take() {
                pending.remove();
            }

            let editor_for_timeout = Rc::clone(&editor);
            let on_properties = Rc::clone(&on_properties);
            state.pending = Some(glib::timeout_add_local_once(PROPERTY_DEBOUNCE, move || {
                let (id, overrides) = {
                    let mut guard = editor_for_timeout.borrow_mut();
                    let Some(state) = guard.as_mut() else {
                        return;
                    };
                    state.pending = None;
                    let overrides: PropertyValues = state
                        .values
                        .iter()
                        .filter(|(key, value)| state.defaults.get(*key) != Some(*value))
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect();

                    (state.wallpaper.clone(), overrides)
                };

                if let Some(handler) = on_properties.borrow().as_ref() {
                    handler(id, overrides);
                }
            }));
        }
    }
}

fn apply_button(active: bool) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .label(if active { "Applied" } else { "Apply" })
        .valign(gtk4::Align::Center)
        .sensitive(!active)
        .build();
    button.add_css_class("suggested-action");

    button
}

fn clear_group(group: &adw::PreferencesGroup) {
    // PreferencesGroup has no "remove all"; its rows live in an internal list box.
    let mut rows = Vec::new();
    let mut child = group.first_child();

    while let Some(widget) = child {
        child = widget.next_sibling();
        collect_rows(&widget, &mut rows);
    }

    for row in rows {
        group.remove(&row);
    }
}

fn collect_rows(widget: &gtk4::Widget, rows: &mut Vec<gtk4::Widget>) {
    if widget.is::<adw::PreferencesRow>() {
        rows.push(widget.clone());

        return;
    }

    let mut child = widget.first_child();

    while let Some(current) = child {
        child = current.next_sibling();
        collect_rows(&current, rows);
    }
}

fn subtitle_for(wallpaper: &Wallpaper) -> String {
    let mut parts = vec![wallpaper.kind.label().to_owned()];

    if let Some(rating) = &wallpaper.content_rating {
        parts.push(rating.clone());
    }

    if !wallpaper.tags.is_empty() {
        parts.push(wallpaper.tags.join(", "));
    }

    parts.join(" · ")
}

fn unsupported_text(reason: &UnsupportedReason) -> String {
    match reason {
        UnsupportedReason::ApplicationWallpaper => {
            "Application wallpapers are Windows programs and cannot run here.".to_owned()
        }
        UnsupportedReason::MissingKind => {
            "The project does not say what kind of wallpaper it is.".to_owned()
        }
        UnsupportedReason::UnknownKind(kind) => {
            format!("Wallpaper type “{kind}” is not supported.")
        }
        UnsupportedReason::MissingEntryFile(path) => {
            format!("The wallpaper's main file is missing: {}", path.display())
        }
    }
}

/// Wallpaper Engine labels are often localisation keys such as `ui_browse_properties_scheme_color`.
fn humanise(label: &str) -> String {
    let stripped = label
        .strip_prefix("ui_browse_properties_")
        .or_else(|| label.strip_prefix("ui_"))
        .unwrap_or(label);

    if stripped == label && label.contains(' ') {
        return label.to_owned();
    }

    let mut words = stripped.split('_').filter(|word| !word.is_empty());
    let mut result = String::new();

    if let Some(first) = words.next() {
        let mut characters = first.chars();

        if let Some(initial) = characters.next() {
            result.extend(initial.to_uppercase());
            result.push_str(characters.as_str());
        }
    }

    for word in words {
        result.push(' ');
        result.push_str(word);
    }

    if result.is_empty() {
        label.to_owned()
    } else {
        result
    }
}
