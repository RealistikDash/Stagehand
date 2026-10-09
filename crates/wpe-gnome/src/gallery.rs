//! The wallpaper grid with search and filters.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk4::gdk;
use gtk4::prelude::*;
use libadwaita as adw;
use wpe_gnome_core::model::LibraryItem;
use wpe_gnome_core::model::Support;
use wpe_gnome_core::model::WallpaperId;
use wpe_gnome_core::model::WallpaperKind;
use wpe_gnome_core::steam::Library;

use crate::preview;
use crate::preview::Animation;

const CARD_WIDTH: i32 = 220;
const CARD_HEIGHT: i32 = 124;
// Decoding at the card width keeps the natural size of a card at one column.
const PREVIEW_DECODE_WIDTH: i32 = CARD_WIDTH;
const ANY: &str = "Any";

type SelectHandler = Box<dyn Fn(WallpaperId)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KindFilter {
    All,
    Scene,
    Video,
    Web,
    Unsupported,
}

struct Card {
    child: gtk4::FlowBoxChild,
    id: WallpaperId,
    haystack: String,
    tags: Vec<String>,
    rating: Option<String>,
    kind: Option<WallpaperKind>,
    supported: bool,
}

pub struct Gallery {
    pub widget: gtk4::Box,
    search_bar: gtk4::SearchBar,
    search: gtk4::SearchEntry,
    kinds: adw::ToggleGroup,
    tags: gtk4::DropDown,
    ratings: gtk4::DropDown,
    flow: gtk4::FlowBox,
    status: adw::StatusPage,
    cards: Rc<RefCell<Vec<Rc<Card>>>>,
    on_select: Rc<RefCell<Option<SelectHandler>>>,
}

impl Gallery {
    pub fn new() -> Self {
        let search = gtk4::SearchEntry::builder()
            .placeholder_text("Search wallpapers")
            .hexpand(true)
            .build();
        let search_bar = gtk4::SearchBar::builder()
            .child(&search)
            .search_mode_enabled(false)
            .build();
        search_bar.connect_entry(&search);

        let kinds = adw::ToggleGroup::new();
        for (name, label) in [
            ("all", "All"),
            ("scene", "Scenes"),
            ("video", "Videos"),
            ("web", "Web"),
            ("unsupported", "Unsupported"),
        ] {
            let toggle = adw::Toggle::builder().name(name).label(label).build();
            kinds.add(toggle);
        }
        kinds.set_active_name(Some("all"));

        let tags = gtk4::DropDown::from_strings(&[ANY]);
        tags.set_tooltip_text(Some("Tag"));
        let ratings = gtk4::DropDown::from_strings(&[ANY]);
        ratings.set_tooltip_text(Some("Content rating"));

        let filters = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(8)
            .margin_bottom(8)
            .build();
        filters.append(&kinds);
        filters.append(&gtk4::Box::builder().hexpand(true).build());
        filters.append(&tags);
        filters.append(&ratings);

        let flow = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .homogeneous(true)
            .column_spacing(12)
            .row_spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(12)
            .min_children_per_line(2)
            .max_children_per_line(8)
            .valign(gtk4::Align::Start)
            .activate_on_single_click(true)
            .build();
        let status = adw::StatusPage::builder()
            .icon_name("image-x-generic-symbolic")
            .title("No Wallpapers Found")
            .description("Subscribe to wallpapers in the Steam Workshop; they appear here once Steam has downloaded them.")
            .visible(false)
            .build();
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&flow)
            .build();
        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        widget.append(&search_bar);
        widget.append(&filters);
        widget.append(&status);
        widget.append(&scroller);

        let gallery = Self {
            widget,
            search_bar,
            search,
            kinds,
            tags,
            ratings,
            flow,
            status,
            cards: Rc::new(RefCell::new(Vec::new())),
            on_select: Rc::new(RefCell::new(None)),
        };
        gallery.install_filter();

        gallery
    }

    pub fn bind_search_toggle(&self, button: &gtk4::ToggleButton) {
        button
            .bind_property("active", &self.search_bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();
    }

    pub fn set_on_select(&self, handler: impl Fn(WallpaperId) + 'static) {
        *self.on_select.borrow_mut() = Some(Box::new(handler));
    }

    fn install_filter(&self) {
        let cards = Rc::clone(&self.cards);
        let search = self.search.clone();
        let kinds = self.kinds.clone();
        let tags = self.tags.clone();
        let ratings = self.ratings.clone();

        self.flow.set_filter_func(move |child| {
            let cards = cards.borrow();
            let Some(card) = cards.iter().find(|card| card.child == *child) else {
                return false;
            };
            let needle = search.text().to_lowercase();
            let kind = match kinds.active_name().as_deref() {
                Some("scene") => KindFilter::Scene,
                Some("video") => KindFilter::Video,
                Some("web") => KindFilter::Web,
                Some("unsupported") => KindFilter::Unsupported,
                _ => KindFilter::All,
            };
            let tag = selected_string(&tags);
            let rating = selected_string(&ratings);

            card_matches(card, &needle, kind, tag.as_deref(), rating.as_deref())
        });

        let flow = self.flow.clone();
        self.search
            .connect_search_changed(move |_| flow.invalidate_filter());
        let flow = self.flow.clone();
        self.kinds
            .connect_active_name_notify(move |_| flow.invalidate_filter());
        let flow = self.flow.clone();
        self.tags
            .connect_selected_notify(move |_| flow.invalidate_filter());
        let flow = self.flow.clone();
        self.ratings
            .connect_selected_notify(move |_| flow.invalidate_filter());

        let on_select = Rc::clone(&self.on_select);
        let cards = Rc::clone(&self.cards);
        self.flow.connect_child_activated(move |_, child| {
            let id = cards
                .borrow()
                .iter()
                .find(|card| card.child == *child)
                .map(|card| card.id.clone());

            if let (Some(id), Some(handler)) = (id, on_select.borrow().as_ref()) {
                handler(id);
            }
        });
    }

    pub fn populate(&self, library: &Library) {
        while let Some(child) = self.flow.first_child() {
            self.flow.remove(&child);
        }

        let mut cards = Vec::new();
        let mut all_tags = BTreeSet::new();
        let mut all_ratings = BTreeSet::new();

        for item in &library.items {
            let card = build_card(item);
            self.flow.append(&card.child);
            all_tags.extend(card.tags.iter().cloned());
            all_ratings.extend(card.rating.iter().cloned());
            cards.push(Rc::new(card));
        }

        *self.cards.borrow_mut() = cards;
        self.status.set_visible(library.items.is_empty());
        refill(&self.tags, all_tags);
        refill(&self.ratings, all_ratings);
        self.flow.invalidate_filter();
    }
}

fn selected_string(dropdown: &gtk4::DropDown) -> Option<String> {
    let selected = dropdown.selected_item()?;
    let text = selected
        .downcast_ref::<gtk4::StringObject>()?
        .string()
        .to_string();

    if text == ANY { None } else { Some(text) }
}

fn refill(dropdown: &gtk4::DropDown, values: BTreeSet<String>) {
    let previous = selected_string(dropdown);
    let mut entries = vec![ANY.to_owned()];
    entries.extend(values);
    let model = gtk4::StringList::new(&entries.iter().map(String::as_str).collect::<Vec<_>>());
    dropdown.set_model(Some(&model));

    if let Some(previous) = previous
        && let Some(index) = entries.iter().position(|entry| *entry == previous)
    {
        dropdown.set_selected(u32::try_from(index).unwrap_or(0));
    }
}

fn card_matches(
    card: &Card,
    needle: &str,
    kind: KindFilter,
    tag: Option<&str>,
    rating: Option<&str>,
) -> bool {
    let kind_ok = match kind {
        KindFilter::All => true,
        KindFilter::Scene => card.kind == Some(WallpaperKind::Scene) && card.supported,
        KindFilter::Video => card.kind == Some(WallpaperKind::Video) && card.supported,
        KindFilter::Web => card.kind == Some(WallpaperKind::Web) && card.supported,
        KindFilter::Unsupported => !card.supported,
    };
    let tag_ok = tag.is_none_or(|tag| card.tags.iter().any(|candidate| candidate == tag));
    let rating_ok = rating.is_none_or(|rating| card.rating.as_deref() == Some(rating));
    let text_ok = needle.is_empty() || card.haystack.contains(needle);

    kind_ok && tag_ok && rating_ok && text_ok
}

fn build_card(item: &LibraryItem) -> Card {
    let picture = gtk4::Picture::builder()
        .content_fit(gtk4::ContentFit::Cover)
        .width_request(CARD_WIDTH)
        .height_request(CARD_HEIGHT)
        .overflow(gtk4::Overflow::Hidden)
        .hexpand(false)
        .build();
    picture.add_css_class("card");
    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&picture));

    let badge = gtk4::Label::builder()
        .halign(gtk4::Align::End)
        .valign(gtk4::Align::Start)
        .margin_top(6)
        .margin_end(6)
        .build();
    badge.add_css_class("osd");
    badge.add_css_class("caption");
    badge.add_css_class("badge");
    overlay.add_overlay(&badge);

    let title = gtk4::Label::builder()
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(24)
        .lines(2)
        .wrap(true)
        .xalign(0.0)
        .margin_top(6)
        .build();
    title.add_css_class("caption-heading");

    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    container.set_width_request(CARD_WIDTH);
    container.append(&overlay);
    container.append(&title);
    let child = gtk4::FlowBoxChild::builder().child(&container).build();

    match item {
        LibraryItem::Wallpaper(wallpaper) => {
            title.set_text(&wallpaper.title);
            let supported = wallpaper.support == Support::Supported;
            badge.set_text(wallpaper.kind.label());

            if !supported {
                badge.set_text("Unsupported");
                badge.add_css_class("error");
                child.add_css_class("dim-label");
            }

            child.set_tooltip_text(Some(&wallpaper.title));
            attach_preview(&picture, wallpaper.preview.clone(), &child);

            Card {
                child,
                id: wallpaper.id.clone(),
                haystack: format!("{} {}", wallpaper.title, wallpaper.tags.join(" "))
                    .to_lowercase(),
                tags: wallpaper.tags.clone(),
                rating: wallpaper.content_rating.clone(),
                kind: Some(wallpaper.kind.clone()),
                supported,
            }
        }
        LibraryItem::Broken(broken) => {
            title.set_text(item.title());
            badge.set_text("Broken");
            badge.add_css_class("error");
            child.set_tooltip_text(Some(&broken.error));
            picture.set_paintable(None::<&gdk::Paintable>);

            Card {
                child,
                id: broken.id.clone(),
                haystack: item.title().to_lowercase(),
                tags: Vec::new(),
                rating: None,
                kind: None,
                supported: false,
            }
        }
    }
}

fn attach_preview(
    picture: &gtk4::Picture,
    preview: Option<std::path::PathBuf>,
    child: &gtk4::FlowBoxChild,
) {
    let Some(path) = preview else {
        return;
    };
    let picture_for_load = picture.clone();
    let still: Rc<RefCell<Option<gdk::Texture>>> = Rc::new(RefCell::new(None));
    let still_for_load = Rc::clone(&still);
    let load_path = path.clone();

    glib::spawn_future_local(async move {
        if let Some(texture) = preview::load_texture(load_path, PREVIEW_DECODE_WIDTH).await {
            picture_for_load.set_paintable(Some(&texture));
            *still_for_load.borrow_mut() = Some(texture);
        }
    });

    if !preview::is_animated(&path) {
        return;
    }

    // Animated GIF previews play only while the pointer is over the card.
    let motion = gtk4::EventControllerMotion::new();
    let running: Rc<RefCell<Option<Animation>>> = Rc::new(RefCell::new(None));
    let enter_path = path.clone();
    let enter_picture = picture.clone();
    let enter_running = Rc::clone(&running);
    motion.connect_enter(move |_, _, _| {
        let mut running = enter_running.borrow_mut();

        if running.is_none() {
            *running = Animation::start(&enter_path, &enter_picture);
        }
    });
    let leave_picture = picture.clone();
    motion.connect_leave(move |_| {
        if let Some(animation) = running.borrow_mut().take() {
            animation.stop(&leave_picture, still.borrow().as_ref());
        }
    });
    child.add_controller(motion);
}
