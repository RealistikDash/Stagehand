use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use crate::config::PropertyValues;
use crate::model::AssignmentLayout;
use crate::model::Assignments;
use crate::model::ConnectorName;
use crate::model::MonitorInfo;
use crate::model::PauseTrigger;
use crate::model::Rectangle;
use crate::model::Settings;
use crate::model::Size;
use crate::model::Support;
use crate::model::UnsupportedReason;
use crate::model::Wallpaper;
use crate::model::WallpaperChoice;
use crate::model::WallpaperKind;
use crate::ports::LaunchKind;
use crate::ports::LaunchPlan;
use crate::ports::SceneLaunch;
use crate::protocol::RendererArguments;
use crate::protocol::RendererContent;
use crate::service::events::Occlusion;
use crate::service::events::ServiceError;

/// One renderer the configuration asks for: a monitor, or the span group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesiredSlot {
    pub connectors: Vec<ConnectorName>,
    pub size: Size,
    pub choice: WallpaperChoice,
}

#[must_use]
pub fn desired_slots(assignments: &Assignments, monitors: &[MonitorInfo]) -> Vec<DesiredSlot> {
    match assignments.layout {
        AssignmentLayout::Span => span_slot(assignments, monitors).into_iter().collect(),
        AssignmentLayout::PerMonitor => monitors
            .iter()
            .filter_map(|monitor| {
                let choice = assignments
                    .monitors
                    .get(&monitor.identity.key())
                    .or_else(|| assignments.monitors.get(&monitor.identity.connector_key()))?;

                Some(DesiredSlot {
                    connectors: vec![monitor.identity.connector.clone()],
                    size: monitor.logical.size(),
                    choice: choice.clone(),
                })
            })
            .collect(),
    }
}

fn span_slot(assignments: &Assignments, monitors: &[MonitorInfo]) -> Option<DesiredSlot> {
    let choice = assignments.span.clone()?;
    let mut rectangles = monitors.iter().map(|monitor| monitor.logical);
    let first = rectangles.next()?;
    let bounds = rectangles.fold(first, Rectangle::union);

    Some(DesiredSlot {
        connectors: monitors
            .iter()
            .map(|monitor| monitor.identity.connector.clone())
            .collect(),
        size: bounds.size(),
        choice,
    })
}

/// Project defaults overlaid with the user's saved values, for renderers that take the full set.
#[must_use]
pub fn merged_properties(
    wallpaper: &Wallpaper,
    overrides: Option<&PropertyValues>,
) -> PropertyValues {
    let mut values: PropertyValues = wallpaper
        .properties
        .iter()
        .filter_map(|property| {
            property
                .default
                .clone()
                .map(|default| (property.key.clone(), default))
        })
        .collect();

    if let Some(overrides) = overrides {
        values.extend(
            overrides
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }

    values
}

pub fn launch_plan(
    wallpaper: &Wallpaper,
    assets_dir: Option<&Path>,
    settings: &Settings,
    overrides: Option<&PropertyValues>,
    slot: &DesiredSlot,
) -> Result<LaunchPlan, ServiceError> {
    if let Support::Unsupported(reason) = &wallpaper.support {
        return Err(ServiceError::Unsupported(
            wallpaper.id.clone(),
            describe_reason(reason),
        ));
    }

    let Some(entry) = wallpaper.entry_file.clone() else {
        return Err(ServiceError::Unsupported(
            wallpaper.id.clone(),
            "the project has no entry file".to_owned(),
        ));
    };
    let scaling = slot.choice.scaling.unwrap_or(settings.scaling);
    let label = format!(
        "stagehand {} {}",
        wallpaper.id,
        connectors_label(&slot.connectors)
    );
    let kind = match wallpaper.kind {
        WallpaperKind::Scene => {
            let Some(assets_dir) = assets_dir else {
                return Err(ServiceError::Unsupported(
                    wallpaper.id.clone(),
                    "the Wallpaper Engine assets directory was not found".to_owned(),
                ));
            };

            LaunchKind::Scene(SceneLaunch {
                directory: wallpaper.directory.clone(),
                assets_dir: assets_dir.to_path_buf(),
                properties: overrides
                    .map(|values| {
                        values
                            .iter()
                            .map(|(key, value)| (key.clone(), value.to_argument()))
                            .collect()
                    })
                    .unwrap_or_default(),
                fps: settings.fps,
                scaling,
                muted: settings.audio.muted,
                volume: settings.audio.volume,
                mute_when_other_app_plays: settings.audio.mute_when_other_app_plays,
            })
        }
        WallpaperKind::Video | WallpaperKind::Web => LaunchKind::Gtk(RendererArguments {
            content: if wallpaper.kind == WallpaperKind::Video {
                RendererContent::Video { file: entry }
            } else {
                RendererContent::Web { index: entry }
            },
            size: slot.size,
            scaling,
            fps: settings.fps,
            muted: settings.audio.muted,
            volume: settings.audio.volume,
            properties: merged_properties(wallpaper, overrides),
            label: label.clone(),
        }),
        WallpaperKind::Application | WallpaperKind::Unknown(_) => {
            return Err(ServiceError::Unsupported(
                wallpaper.id.clone(),
                describe_reason(&UnsupportedReason::ApplicationWallpaper),
            ));
        }
    };

    Ok(LaunchPlan {
        kind,
        size: slot.size,
        connectors: slot.connectors.clone(),
        label,
    })
}

/// Whether switching from `current` to `next` needs a new process. Audio-only changes are
/// applied live to the GTK renderer; linux-wallpaperengine has no live channel.
#[must_use]
pub fn requires_restart(current: &LaunchPlan, next: &LaunchPlan) -> bool {
    match (&current.kind, &next.kind) {
        (LaunchKind::Gtk(current_arguments), LaunchKind::Gtk(next_arguments)) => {
            let mut comparable = next_arguments.clone();
            comparable.muted = current_arguments.muted;
            comparable.volume = current_arguments.volume;

            current.connectors != next.connectors
                || current.size != next.size
                || *current_arguments != comparable
        }
        (LaunchKind::Scene(_), LaunchKind::Scene(_) | LaunchKind::Gtk(_))
        | (LaunchKind::Gtk(_), LaunchKind::Scene(_)) => current != next,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Conditions {
    pub locked: bool,
    pub idle: bool,
    pub on_battery: bool,
}

/// The enabled triggers that currently apply to a slot. A spanned slot pauses only when every
/// one of its monitors is covered.
#[must_use]
pub fn pause_reasons(
    settings: &Settings,
    conditions: Conditions,
    occlusion: &BTreeMap<ConnectorName, Occlusion>,
    connectors: &[ConnectorName],
) -> Vec<PauseTrigger> {
    let mut reasons = BTreeSet::new();
    let enabled = &settings.pause_on;

    if conditions.locked && enabled.contains(&PauseTrigger::Lock) {
        reasons.insert(PauseTrigger::Lock);
    }

    if conditions.idle && enabled.contains(&PauseTrigger::Idle) {
        reasons.insert(PauseTrigger::Idle);
    }

    if conditions.on_battery && enabled.contains(&PauseTrigger::Battery) {
        reasons.insert(PauseTrigger::Battery);
    }

    let states: Vec<Occlusion> = connectors
        .iter()
        .map(|connector| occlusion.get(connector).copied().unwrap_or_default())
        .collect();
    let all_fullscreen =
        !states.is_empty() && states.iter().all(|state| *state == Occlusion::Fullscreen);
    let all_covered = !states.is_empty() && states.iter().all(|state| *state != Occlusion::None);

    if all_fullscreen && enabled.contains(&PauseTrigger::Fullscreen) {
        reasons.insert(PauseTrigger::Fullscreen);
    } else if all_covered && enabled.contains(&PauseTrigger::Maximised) {
        reasons.insert(PauseTrigger::Maximised);
    }

    reasons.into_iter().collect()
}

fn describe_reason(reason: &UnsupportedReason) -> String {
    match reason {
        UnsupportedReason::ApplicationWallpaper => {
            "application wallpapers are Windows executables".to_owned()
        }
        UnsupportedReason::MissingKind => "the project has no type".to_owned(),
        UnsupportedReason::UnknownKind(kind) => format!("unknown wallpaper type `{kind}`"),
        UnsupportedReason::MissingEntryFile(path) => {
            format!("the entry file {} is missing", path.display())
        }
    }
}

fn connectors_label(connectors: &[ConnectorName]) -> String {
    connectors
        .iter()
        .map(ConnectorName::as_str)
        .collect::<Vec<_>>()
        .join("+")
}
