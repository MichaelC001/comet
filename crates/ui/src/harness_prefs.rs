//! Which harnesses the user has enabled (Settings → Agents).
//!
//! Follows the [`crate::appearance`] plumbing: a gpui global seeded from
//! `ui-settings.json` at boot, mutated by the settings page, persisted with a
//! read-modify-write of just this key (the shell's debounced save holds its
//! own snapshot — writing that from here would roll back newer pane state).
//!
//! The set gates the composer's harness rail and the eager model prefetch;
//! a chat already committed to a since-disabled harness keeps working — the
//! filter applies to *offering* a harness, never to running one.

use std::path::PathBuf;

use gpui::{App, Global};

use comet_proto::HarnessId;

use crate::settings::{UiSettings, default_enabled_harnesses};

pub struct HarnessPrefs {
    /// `None` = the user never touched the setting → the default set.
    enabled: Option<Vec<HarnessId>>,
    data_dir: PathBuf,
}

impl Global for HarnessPrefs {}

/// Install the global. Call once at boot, before the first composer render
/// (the pickers read it to filter the rail and scope the model prefetch).
pub fn init(enabled: Option<Vec<HarnessId>>, data_dir: impl Into<PathBuf>, cx: &mut App) {
    cx.set_global(HarnessPrefs {
        enabled,
        data_dir: data_dir.into(),
    });
}

/// The enabled set currently in effect (the default set before [`init`]).
pub fn enabled(cx: &App) -> Vec<HarnessId> {
    cx.try_global::<HarnessPrefs>()
        .and_then(|prefs| prefs.enabled.clone())
        .unwrap_or_else(default_enabled_harnesses)
}

pub fn is_enabled(harness: HarnessId, cx: &App) -> bool {
    enabled(cx).contains(&harness)
}

/// Flip one harness, persist, and repaint. Disabling the last enabled harness
/// is refused — an empty rail would leave the composer with nothing to run
/// (the settings page disables that toggle too; this is the backstop).
pub fn set_enabled(harness: HarnessId, on: bool, cx: &mut App) {
    if !cx.has_global::<HarnessPrefs>() {
        return;
    }
    let mut set = enabled(cx);
    match (on, set.contains(&harness)) {
        (true, false) => set.push(harness),
        (false, true) => {
            if set.len() == 1 {
                return;
            }
            set.retain(|h| *h != harness);
        }
        _ => return,
    }
    let prefs = cx.global_mut::<HarnessPrefs>();
    prefs.enabled = Some(set.clone());
    let data_dir = prefs.data_dir.clone();
    persist(set, &data_dir);
    // Blunt but rare: the pickers read the set at render time with no
    // subscription to invalidate them, so repaint everything.
    cx.refresh_windows();
}

/// Read-modify-write `ui-settings.json` for just the harness key (see
/// [`crate::appearance::set_mode`] for why not a cached snapshot).
fn persist(set: Vec<HarnessId>, data_dir: &std::path::Path) {
    let mut settings = UiSettings::load(data_dir);
    settings.enabled_harnesses = Some(set);
    if let Err(err) = settings.save(data_dir) {
        tracing::warn!(error = %err, "could not persist enabled harnesses");
    }
}
