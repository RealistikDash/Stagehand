//! The monitors page: every connected monitor with its current wallpaper and state.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use stagehand_core::config::Config;
use stagehand_core::model::AssignmentLayout;
use stagehand_core::model::LibraryItem;
use stagehand_core::model::MonitorInfo;
use stagehand_core::model::MonitorKey;
use stagehand_core::model::PauseTrigger;
use stagehand_core::model::SlotState;
use stagehand_core::model::SlotStatus;
use stagehand_core::model::Status;
use stagehand_core::steam::Library;

type UnassignHandler = Box<dyn Fn(MonitorKey)>;
type LayoutHandler = Box<dyn Fn(AssignmentLayout)>;
type StopHandler = Box<dyn Fn()>;

pub struct MonitorsPage {
    pub widget: gtk4::ScrolledWindow,
    group: adw::PreferencesGroup,
    span_row: adw::SwitchRow,
    span_subtitle: adw::ActionRow,
    on_unassign: Rc<RefCell<Option<UnassignHandler>>>,
    on_layout: Rc<RefCell<Option<LayoutHandler>>>,
    on_stop: Rc<RefCell<Option<StopHandler>>>,
    updating: Rc<RefCell<bool>>,
}

impl MonitorsPage {
    pub fn new() -> Self {
        let stop = gtk4::Button::builder()
            .label("Stop All")
            .valign(gtk4::Align::Center)
            .tooltip_text("Remove every assignment and show the static background")
            .build();
        stop.add_css_class("destructive-action");
        let group = adw::PreferencesGroup::builder()
            .title("Monitors")
            .description("Pick a wallpaper in the library and apply it to a monitor.")
            .header_suffix(&stop)
            .build();
        let span_row = adw::SwitchRow::builder()
            .title("Span one wallpaper across all monitors")
            .subtitle("The spanned wallpaper is chosen with “All monitors” in the library")
            .build();
        let span_subtitle = adw::ActionRow::builder().title("Spanned wallpaper").build();
        let layout_group = adw::PreferencesGroup::builder().title("Layout").build();
        layout_group.add(&span_row);
        layout_group.add(&span_subtitle);

        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(24)
            .margin_top(24)
            .margin_bottom(24)
            .build();
        content.append(&group);
        content.append(&layout_group);
        let clamp = adw::Clamp::builder()
            .child(&content)
            .maximum_size(720)
            .build();
        let widget = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&clamp)
            .build();

        let page = Self {
            widget,
            group,
            span_row,
            span_subtitle,
            on_unassign: Rc::new(RefCell::new(None)),
            on_layout: Rc::new(RefCell::new(None)),
            on_stop: Rc::new(RefCell::new(None)),
            updating: Rc::new(RefCell::new(false)),
        };

        let on_stop = Rc::clone(&page.on_stop);
        stop.connect_clicked(move |_| {
            if let Some(handler) = on_stop.borrow().as_ref() {
                handler();
            }
        });

        let on_layout = Rc::clone(&page.on_layout);
        let updating = Rc::clone(&page.updating);
        page.span_row.connect_active_notify(move |row| {
            if *updating.borrow() {
                return;
            }

            let layout = if row.is_active() {
                AssignmentLayout::Span
            } else {
                AssignmentLayout::PerMonitor
            };

            if let Some(handler) = on_layout.borrow().as_ref() {
                handler(layout);
            }
        });

        page
    }

    pub fn set_on_unassign(&self, handler: impl Fn(MonitorKey) + 'static) {
        *self.on_unassign.borrow_mut() = Some(Box::new(handler));
    }

    pub fn set_on_layout(&self, handler: impl Fn(AssignmentLayout) + 'static) {
        *self.on_layout.borrow_mut() = Some(Box::new(handler));
    }

    pub fn set_on_stop(&self, handler: impl Fn() + 'static) {
        *self.on_stop.borrow_mut() = Some(Box::new(handler));
    }

    pub fn update(
        &self,
        monitors: &[MonitorInfo],
        status: &Status,
        config: &Config,
        library: &Library,
    ) {
        *self.updating.borrow_mut() = true;
        self.span_row
            .set_active(config.assignments.layout == AssignmentLayout::Span);
        *self.updating.borrow_mut() = false;
        self.span_subtitle.set_subtitle(
            &config
                .assignments
                .span
                .as_ref()
                .and_then(|choice| library.find(&choice.wallpaper))
                .map_or("None".to_owned(), |item| item.title().to_owned()),
        );

        let mut child = self.group.first_child();
        let mut rows = Vec::new();

        while let Some(widget) = child {
            child = widget.next_sibling();
            collect_rows(&widget, &mut rows);
        }

        for row in rows {
            self.group.remove(&row);
        }

        for monitor in monitors {
            let slot = status
                .slots
                .iter()
                .find(|slot| slot.connectors.contains(&monitor.identity.connector));
            let wallpaper_title = slot
                .and_then(|slot| slot.wallpaper.as_ref())
                .and_then(|id| library.find(id))
                .map_or("No wallpaper", LibraryItem::title);
            let state = slot.map_or("static background".to_owned(), describe_state);
            let row = adw::ActionRow::builder()
                .title(format!(
                    "{} {} ({})",
                    monitor.identity.vendor, monitor.identity.product, monitor.identity.connector
                ))
                .subtitle(format!(
                    "{}×{} at {},{} · scale {} · {wallpaper_title} · {state}",
                    monitor.logical.width,
                    monitor.logical.height,
                    monitor.logical.x,
                    monitor.logical.y,
                    monitor.scale,
                ))
                .build();

            if monitor.is_primary {
                let badge = gtk4::Label::new(Some("Primary"));
                badge.add_css_class("dim-label");
                badge.set_valign(gtk4::Align::Center);
                row.add_suffix(&badge);
            }

            let key = monitor.identity.key();
            let assigned = config.assignments.monitors.contains_key(&key)
                || config
                    .assignments
                    .monitors
                    .contains_key(&monitor.identity.connector_key());
            let remove = gtk4::Button::builder()
                .icon_name("edit-delete-symbolic")
                .valign(gtk4::Align::Center)
                .tooltip_text("Remove the wallpaper from this monitor")
                .sensitive(assigned)
                .build();
            remove.add_css_class("flat");
            let on_unassign = Rc::clone(&self.on_unassign);
            let connector_key = monitor.identity.connector_key();
            remove.connect_clicked(move |_| {
                if let Some(handler) = on_unassign.borrow().as_ref() {
                    handler(key.clone());
                    handler(connector_key.clone());
                }
            });
            row.add_suffix(&remove);
            self.group.add(&row);
        }
    }
}

fn describe_state(slot: &SlotStatus) -> String {
    match &slot.state {
        SlotState::Idle => "idle".to_owned(),
        SlotState::Starting => "starting".to_owned(),
        SlotState::Running => "playing".to_owned(),
        SlotState::Paused { reasons, hidden } => {
            let why: Vec<&str> = reasons
                .iter()
                .map(|reason| trigger_short(*reason))
                .collect();

            format!(
                "paused ({}){}",
                why.join(", "),
                if *hidden { ", still shown" } else { "" }
            )
        }
        SlotState::Failed { message } => format!("failed: {message}"),
    }
}

fn trigger_short(trigger: PauseTrigger) -> &'static str {
    match trigger {
        PauseTrigger::Fullscreen => "fullscreen window",
        PauseTrigger::Maximised => "maximised window",
        PauseTrigger::Lock => "locked",
        PauseTrigger::Idle => "idle",
        PauseTrigger::Battery => "on battery",
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
