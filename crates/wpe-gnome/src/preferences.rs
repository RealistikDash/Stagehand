//! The preferences dialog. Every change is written to the daemon as a whole `Settings` value.

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;

use libadwaita as adw;
use libadwaita::prelude::*;
use wpe_gnome_core::model::DimLevel;
use wpe_gnome_core::model::Fps;
use wpe_gnome_core::model::PauseTrigger;
use wpe_gnome_core::model::ScalingMode;
use wpe_gnome_core::model::Settings;
use wpe_gnome_core::model::Volume;

use crate::autostart;
use crate::window::Ui;

pub fn show(ui: &Rc<Ui>) {
    let settings = Rc::new(RefCell::new(ui.state.borrow().config.settings.clone()));
    let dialog = adw::PreferencesDialog::builder()
        .title("Preferences")
        .build();

    dialog.add(&general_page(ui, &settings));
    dialog.add(&audio_page(ui, &settings));
    dialog.add(&pause_page(ui, &settings));
    dialog.present(Some(&ui.window));
}

fn push(ui: &Rc<Ui>, settings: &Rc<RefCell<Settings>>) {
    let snapshot = settings.borrow().clone();

    ui.call(move |client| async move { client.set_settings(&snapshot).await });
}

fn general_page(ui: &Rc<Ui>, settings: &Rc<RefCell<Settings>>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("General")
        .icon_name("preferences-system-symbolic")
        .build();
    let startup = adw::PreferencesGroup::builder().title("Startup").build();
    let login = adw::SwitchRow::builder()
        .title("Start at login")
        .subtitle("Runs the wallpaper service when you log in")
        .build();
    startup.add(&login);
    page.add(&startup);

    let login_for_query = login.clone();
    let updating = Rc::new(RefCell::new(true));
    let updating_for_query = Rc::clone(&updating);
    glib::spawn_future_local(async move {
        let enabled = autostart::is_enabled().await.unwrap_or(false);
        login_for_query.set_active(enabled);
        *updating_for_query.borrow_mut() = false;
    });
    let ui_for_login = Rc::clone(ui);
    login.connect_active_notify(move |row| {
        if *updating.borrow() {
            return;
        }

        let enabled = row.is_active();
        let ui = Rc::clone(&ui_for_login);

        glib::spawn_future_local(async move {
            if let Err(error) = autostart::set_enabled(enabled).await {
                ui.toast(&error.to_string());
            }
        });
    });

    let rendering = adw::PreferencesGroup::builder().title("Rendering").build();
    let fps = adw::SpinRow::with_range(1.0, f64::from(Fps::MAX), 1.0);
    fps.set_title("Frame rate limit");
    fps.set_subtitle("Scenes honour this exactly; videos are capped to it");
    fps.set_value(f64::from(settings.borrow().fps.get()));
    let (ui_fps, settings_fps) = (Rc::clone(ui), Rc::clone(settings));
    fps.connect_value_notify(move |row| {
        let Some(value) = Fps::new(saturating_u32(row.value())) else {
            return;
        };
        settings_fps.borrow_mut().fps = value;
        push(&ui_fps, &settings_fps);
    });
    rendering.add(&fps);

    let labels: Vec<&str> = ScalingMode::ALL.iter().map(|mode| mode.label()).collect();
    let scaling = adw::ComboRow::builder()
        .title("Scaling")
        .subtitle("How a wallpaper fills a monitor of a different shape")
        .model(&gtk4::StringList::new(&labels))
        .build();
    let current = ScalingMode::ALL
        .iter()
        .position(|mode| *mode == settings.borrow().scaling)
        .unwrap_or(0);
    scaling.set_selected(u32::try_from(current).unwrap_or(0));
    let (ui_scaling, settings_scaling) = (Rc::clone(ui), Rc::clone(settings));
    scaling.connect_selected_notify(move |row| {
        let Some(mode) = ScalingMode::ALL.get(usize::try_from(row.selected()).unwrap_or(0)) else {
            return;
        };
        settings_scaling.borrow_mut().scaling = *mode;
        push(&ui_scaling, &settings_scaling);
    });
    rendering.add(&scaling);
    page.add(&rendering);

    let oled = adw::PreferencesGroup::builder()
        .title("OLED Care")
        .description("Paused wallpapers keep their last frame; after the timeout the still image shows instead. The screen is never kept awake.")
        .build();
    let dim = adw::SpinRow::with_range(0.0, f64::from(DimLevel::MAX_PERCENT), 5.0);
    dim.set_title("Dim wallpaper");
    dim.set_subtitle("Percent darker");
    dim.set_value(f64::from(settings.borrow().oled.dim.percent()));
    let (ui_dim, settings_dim) = (Rc::clone(ui), Rc::clone(settings));
    dim.connect_value_notify(move |row| {
        settings_dim.borrow_mut().oled.dim = DimLevel::new(saturating_u8(row.value()));
        push(&ui_dim, &settings_dim);
    });
    oled.add(&dim);

    let timeout = adw::SpinRow::with_range(0.0, 3600.0, 30.0);
    timeout.set_title("Hide frozen frame after");
    timeout.set_subtitle("Seconds while paused; 0 keeps the frame");
    timeout.set_value(f64::from(
        settings
            .borrow()
            .oled
            .paused_still_timeout_seconds
            .map_or(0, NonZeroU32::get),
    ));
    let (ui_timeout, settings_timeout) = (Rc::clone(ui), Rc::clone(settings));
    timeout.connect_value_notify(move |row| {
        settings_timeout
            .borrow_mut()
            .oled
            .paused_still_timeout_seconds = NonZeroU32::new(saturating_u32(row.value()));
        push(&ui_timeout, &settings_timeout);
    });
    oled.add(&timeout);
    page.add(&oled);

    page
}

fn audio_page(ui: &Rc<Ui>, settings: &Rc<RefCell<Settings>>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Audio")
        .icon_name("audio-speakers-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder()
        .title("Wallpaper Audio")
        .build();

    let muted = adw::SwitchRow::builder().title("Mute wallpapers").build();
    muted.set_active(settings.borrow().audio.muted);
    let (ui_muted, settings_muted) = (Rc::clone(ui), Rc::clone(settings));
    muted.connect_active_notify(move |row| {
        settings_muted.borrow_mut().audio.muted = row.is_active();
        push(&ui_muted, &settings_muted);
    });
    group.add(&muted);

    let volume = adw::SpinRow::with_range(0.0, 100.0, 5.0);
    volume.set_title("Volume");
    volume.set_subtitle("Percent");
    volume.set_value(f64::from(settings.borrow().audio.volume.percent()));
    let (ui_volume, settings_volume) = (Rc::clone(ui), Rc::clone(settings));
    volume.connect_value_notify(move |row| {
        settings_volume.borrow_mut().audio.volume = Volume::new(saturating_u8(row.value()));
        push(&ui_volume, &settings_volume);
    });
    group.add(&volume);

    let other = adw::SwitchRow::builder()
        .title("Mute when another app plays audio")
        .subtitle("Scenes handle this themselves; other wallpapers follow the service")
        .build();
    other.set_active(settings.borrow().audio.mute_when_other_app_plays);
    let (ui_other, settings_other) = (Rc::clone(ui), Rc::clone(settings));
    other.connect_active_notify(move |row| {
        settings_other.borrow_mut().audio.mute_when_other_app_plays = row.is_active();
        push(&ui_other, &settings_other);
    });
    group.add(&other);
    page.add(&group);

    page
}

fn pause_page(ui: &Rc<Ui>, settings: &Rc<RefCell<Settings>>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Pause")
        .icon_name("media-playback-pause-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder()
        .title("Pause the wallpaper when")
        .description("A paused wallpaper uses no CPU or GPU.")
        .build();

    for trigger in PauseTrigger::ALL {
        let row = adw::SwitchRow::builder().title(trigger.label()).build();
        row.set_active(settings.borrow().pause_on.contains(&trigger));
        let (ui_row, settings_row) = (Rc::clone(ui), Rc::clone(settings));
        row.connect_active_notify(move |row| {
            {
                let mut settings = settings_row.borrow_mut();

                if row.is_active() {
                    settings.pause_on.insert(trigger);
                } else {
                    settings.pause_on.remove(&trigger);
                }
            }

            push(&ui_row, &settings_row);
        });
        group.add(&row);
    }

    page.add(&group);

    page
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "spin rows are bounded; saturating conversion is the intent"
)]
fn saturating_u32(value: f64) -> u32 {
    value.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "spin rows are bounded; saturating conversion is the intent"
)]
fn saturating_u8(value: f64) -> u8 {
    value.round().clamp(0.0, f64::from(u8::MAX)) as u8
}
