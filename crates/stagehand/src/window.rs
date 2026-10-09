use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use stagehand_core::config::Config;
use stagehand_core::model::LibraryItem;
use stagehand_core::model::MonitorInfo;
use stagehand_core::model::Status;
use stagehand_core::model::WallpaperChoice;
use stagehand_core::model::WallpaperId;
use stagehand_core::steam::Library;

use crate::client::Change;
use crate::client::ClientError;
use crate::client::DaemonClient;
use crate::detail::ApplyTarget;
use crate::detail::DetailPane;
use crate::gallery::Gallery;
use crate::monitors::MonitorsPage;
use crate::preferences;

const WINDOW_TITLE: &str = "Stagehand";

#[derive(Debug, Default)]
pub struct State {
    pub library: Library,
    pub monitors: Vec<MonitorInfo>,
    pub status: Status,
    pub config: Config,
    pub selected: Option<WallpaperId>,
}

pub struct Ui {
    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub client: RefCell<Option<Rc<DaemonClient>>>,
    pub state: RefCell<State>,
    gallery: Gallery,
    detail: DetailPane,
    monitors: MonitorsPage,
    split: adw::OverlaySplitView,
    status_banner: adw::Banner,
}

pub fn present(application: &adw::Application) {
    let ui = Rc::new(Ui::build(application));

    ui.wire();
    ui.window.present();
    ui.clone().connect();
}

impl Ui {
    fn build(application: &adw::Application) -> Self {
        let gallery = Gallery::new();
        let detail = DetailPane::new();
        let monitors = MonitorsPage::new();
        let split = adw::OverlaySplitView::builder()
            .content(&gallery.widget)
            .sidebar(&detail.widget)
            .sidebar_position(gtk4::PackType::End)
            .show_sidebar(false)
            .min_sidebar_width(320.0)
            .max_sidebar_width(420.0)
            .sidebar_width_fraction(0.33)
            .build();
        let stack = adw::ViewStack::new();
        stack
            .add_titled(&split, Some("library"), "Library")
            .set_icon_name(Some("image-x-generic-symbolic"));
        stack
            .add_titled(&monitors.widget, Some("monitors"), "Monitors")
            .set_icon_name(Some("video-display-symbolic"));

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        let menu = gio::Menu::new();
        menu.append(Some("_Preferences"), Some("app.preferences"));
        menu.append(Some("_Rescan Library"), Some("app.rescan"));
        menu.append(Some("_About Stagehand"), Some("app.about"));
        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .primary(true)
            .build();
        let search_button = gtk4::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text("Search")
            .build();
        gallery.bind_search_toggle(&search_button);
        let header = adw::HeaderBar::builder().title_widget(&switcher).build();
        header.pack_start(&search_button);
        header.pack_end(&menu_button);

        let status_banner = adw::Banner::builder().revealed(false).build();
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content.append(&status_banner);
        content.append(&stack);
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar));
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title(WINDOW_TITLE)
            .default_width(1100)
            .default_height(720)
            .content(&toasts)
            .build();

        Self {
            window,
            toasts,
            client: RefCell::new(None),
            state: RefCell::new(State::default()),
            gallery,
            detail,
            monitors,
            split,
            status_banner,
        }
    }

    fn wire(self: &Rc<Self>) {
        let ui = Rc::clone(self);
        self.gallery.set_on_select(move |id| ui.select(&id));

        let ui = Rc::clone(self);
        self.detail
            .set_on_close(move || ui.split.set_show_sidebar(false));

        let ui = Rc::clone(self);
        self.detail
            .set_on_apply(move |target, choice| ui.apply(target, choice));

        let ui = Rc::clone(self);
        self.detail
            .set_on_properties(move |id, values| ui.save_properties(id, values));

        let ui = Rc::clone(self);
        self.monitors.set_on_unassign(move |key| {
            ui.call(move |client| async move { client.unassign(&key).await });
        });

        let ui = Rc::clone(self);
        self.monitors.set_on_layout(move |layout| {
            ui.call(move |client| async move { client.set_layout(layout).await });
        });

        let ui = Rc::clone(self);
        self.monitors.set_on_stop(move || {
            ui.call(|client| async move { client.stop().await });
        });

        let application = self.window.application();
        let Some(application) = application.and_downcast::<adw::Application>() else {
            return;
        };
        let preferences_action = gio::SimpleAction::new("preferences", None);
        let ui = Rc::clone(self);
        preferences_action.connect_activate(move |_, _| preferences::show(&ui));
        application.add_action(&preferences_action);
        application.set_accels_for_action("app.preferences", &["<Control>comma"]);

        let rescan_action = gio::SimpleAction::new("rescan", None);
        let ui = Rc::clone(self);
        rescan_action.connect_activate(move |_, _| {
            ui.call(|client| async move { client.rescan().await });
        });
        application.add_action(&rescan_action);

        let about_action = gio::SimpleAction::new("about", None);
        let ui = Rc::clone(self);
        about_action.connect_activate(move |_, _| ui.show_about());
        application.add_action(&about_action);

        let quit_action = gio::SimpleAction::new("quit", None);
        let app = application.clone();
        quit_action.connect_activate(move |_, _| app.quit());
        application.add_action(&quit_action);
        application.set_accels_for_action("app.quit", &["<Control>q"]);
    }

    fn connect(self: Rc<Self>) {
        glib::spawn_future_local(async move {
            let client = match DaemonClient::connect().await {
                Ok(client) => Rc::new(client),
                Err(error) => {
                    self.status_banner.set_title(&error.to_string());
                    self.status_banner.set_revealed(true);

                    return;
                }
            };
            let changes = match client.subscribe().await {
                Ok(changes) => changes,
                Err(error) => {
                    self.toast(&error.to_string());

                    return;
                }
            };
            *self.client.borrow_mut() = Some(Rc::clone(&client));
            self.refresh(Change::Config).await;
            self.refresh(Change::Monitors).await;
            self.refresh(Change::Library).await;
            self.refresh(Change::Status).await;

            while let Ok(change) = changes.recv().await {
                self.refresh(change).await;
            }
        });
    }

    async fn refresh(&self, change: Change) {
        let Some(client) = self.client() else {
            return;
        };
        let result = match change {
            Change::Library => client.library().await.map(|library| {
                self.state.borrow_mut().library = library;
            }),
            Change::Monitors => client.monitors().await.map(|monitors| {
                self.state.borrow_mut().monitors = monitors;
            }),
            Change::Status => client.status().await.map(|status| {
                self.state.borrow_mut().status = status;
            }),
            Change::Config => client.config().await.map(|config| {
                self.state.borrow_mut().config = config;
            }),
        };

        if let Err(error) = result {
            self.toast(&error.to_string());

            return;
        }

        match change {
            Change::Library => {
                let state = self.state.borrow();
                self.gallery.populate(&state.library);
            }
            Change::Monitors | Change::Status | Change::Config => {}
        }

        self.update_pages();
    }

    fn update_pages(&self) {
        let state = self.state.borrow();

        self.monitors.update(
            &state.monitors,
            &state.status,
            &state.config,
            &state.library,
        );
        self.status_banner.set_revealed(!state.status.shell_ready);
        self.status_banner.set_title(
            "The stagehand Shell extension is not running. Enable it in Extensions and log in again.",
        );

        if let Some(selected) = state.selected.clone()
            && let Some(item) = state.library.find(&selected)
        {
            let saved = state.config.property_values.get(&selected);
            self.detail
                .show(item, &state.monitors, &state.status, saved);
        }
    }

    fn select(&self, id: &WallpaperId) {
        {
            let mut state = self.state.borrow_mut();
            state.selected = Some(id.clone());
        }

        let state = self.state.borrow();
        let Some(item) = state.library.find(id) else {
            return;
        };
        let saved = state.config.property_values.get(id);
        self.detail
            .show(item, &state.monitors, &state.status, saved);
        self.split.set_show_sidebar(true);
    }

    fn apply(self: &Rc<Self>, target: ApplyTarget, choice: WallpaperChoice) {
        let title = self
            .state
            .borrow()
            .library
            .find(&choice.wallpaper)
            .map(LibraryItem::title)
            .unwrap_or_default()
            .to_owned();
        let ui = Rc::clone(self);

        self.call(move |client| async move {
            let result = match target {
                ApplyTarget::Monitor(key) => client.assign(&key, &choice).await,
                ApplyTarget::Span => client.assign_span(&choice).await,
            };

            if result.is_ok() {
                ui.toast(&format!("Applied “{title}”"));
            }

            result
        });
    }

    fn save_properties(
        self: &Rc<Self>,
        id: WallpaperId,
        values: stagehand_core::config::PropertyValues,
    ) {
        self.call(move |client| async move { client.set_property_values(&id, &values).await });
    }

    pub fn client(&self) -> Option<Rc<DaemonClient>> {
        self.client.borrow().clone()
    }

    /// Runs one daemon call on the main loop and shows a toast if it fails.
    pub fn call<F, Fut>(self: &Rc<Self>, make: F)
    where
        F: FnOnce(Rc<DaemonClient>) -> Fut + 'static,
        Fut: Future<Output = Result<(), ClientError>> + 'static,
    {
        let Some(client) = self.client() else {
            self.toast("The wallpaper service is not running.");

            return;
        };
        let ui = Rc::clone(self);

        glib::spawn_future_local(async move {
            if let Err(error) = make(client).await {
                ui.toast(&error.to_string());
            }
        });
    }

    pub fn toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }

    fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Stagehand")
            .application_icon("io.github.realistikdash.Stagehand")
            .developer_name("RealistikDash")
            .version(env!("CARGO_PKG_VERSION"))
            .license_type(gtk4::License::MitX11)
            .comments("Steam Wallpaper Engine wallpapers as live GNOME backgrounds. Scenes are rendered by linux-wallpaperengine.")
            .build();

        about.present(Some(&self.window));
    }
}
