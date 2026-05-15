// Per-tag wall-clock time accounting.
//
// Mirrors the JS-era `tag-manager.js` `startTagTracking` /
// `stopTagTracking` / `onTimerStart` / `onTimerPause` / `onTimerStop`
// surface. The `StoredValue<HashMap<String, i64>>` map carries the
// per-tag start-anchor (wall-clock ms via `Date.now()`); flushes
// drain the map and persist each tag's accumulated duration through
// the `add_session_tag` Tauri command.
//
// The pure decision function `tag_tracking_action_for_event` is
// extracted as a host-testable classifier so the start-vs-flush
// matrix is verifiable without the Leptos reactive context.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::now_iso;
use crate::bridge::commands;
use crate::bridge::types::SessionTag;
use crate::components::browser_clock::BrowserClock;
use crate::engine::clock::Clock;
use crate::engine::timer::TimerEvent;

/// Classify an engine event for the tag-tracking subsystem.
///
/// Pure decision function — no reactive context, no side effects.
/// Mirrors the JS-era `tag-manager.js` hooks at lines 552-617:
/// - start/resume → start trackers for every selected tag
/// - pause/auto-pause/completion/skip → flush every active tracker
/// - overtime/warnings/manual entry → no-op (overtime preserves
///   the running trackers; warnings and manual recordings don't
///   touch the active session timing).
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TagTrackingAction {
    /// Start a tracker for every currently-selected tag id.
    StartAllSelected,
    /// Flush every active tracker (saving accumulated durations).
    FlushAll,
    /// No tag-tracking work for this event.
    NoOp,
}

#[must_use]
pub(super) const fn tag_tracking_action_for_event(event: &TimerEvent) -> TagTrackingAction {
    match event {
        TimerEvent::SessionStarted | TimerEvent::SessionResumed | TimerEvent::AutoResumed => {
            TagTrackingAction::StartAllSelected
        }
        TimerEvent::SessionPaused
        | TimerEvent::AutoPaused
        | TimerEvent::PomodoroCompleted { .. }
        | TimerEvent::BreakCompleted { .. }
        | TimerEvent::SessionSkipped { .. }
        | TimerEvent::SessionAborted { .. } => TagTrackingAction::FlushAll,
        TimerEvent::OvertimeStarted { .. }
        | TimerEvent::TwoMinutesRemaining
        | TimerEvent::ThirtySecondsRemaining
        | TimerEvent::ManualSessionRecorded { .. }
        | TimerEvent::SessionCompletedEarly { .. } => TagTrackingAction::NoOp,
    }
}

/// Persist a single `SessionTag` join row through the Tauri bridge.
/// Durations below 10 seconds are dropped to avoid spamming the
/// on-disk log with micro-toggles during the tag-dropdown UI dance.
/// `session_id` is created at tracking-start time so all flushes for
/// the same run segment share one stable identifier.
pub(super) fn save_session_tag(session_id: String, tag_id: String, duration_secs: u32) {
    if duration_secs < 10 {
        return;
    }
    let session_tag = SessionTag {
        session_id,
        tag_id,
        duration: duration_secs,
        created_at: now_iso(),
    };
    spawn_local(async move {
        let _ = commands::add_session_tag(session_tag).await;
    });
}

/// Begin tracking wall-clock time spent on `tag_id` from `now` ms.
/// `session_id` is captured once here and carried through to flush so
/// multiple flush calls for the same run segment use the same id.
/// No-op if the tag is already being tracked (legacy parity:
/// `tag-manager.js:startTagTracking` short-circuits when the key
/// already exists, so re-entering on resume doesn't reset the anchor).
pub(super) fn tag_tracking_start(
    map: StoredValue<HashMap<String, (String, i64)>>,
    tag_id: &str,
    session_id: &str,
    now: i64,
) {
    map.update_value(|m| {
        m.entry(tag_id.to_string())
            .or_insert_with(|| (session_id.to_string(), now));
    });
}

/// Stop tracking `tag_id`, flushing the accumulated duration through
/// `save_session_tag`. Mirrors `tag-manager.js:stopTagTracking`.
pub(super) fn tag_tracking_flush_one(
    map: StoredValue<HashMap<String, (String, i64)>>,
    tag_id: &str,
    now: i64,
) {
    let entry = map.try_update_value(|m| m.remove(tag_id)).flatten();
    if let Some((session_id, start_ms)) = entry {
        let duration = u32::try_from(((now - start_ms) / 1000).max(0)).unwrap_or(0);
        save_session_tag(session_id, tag_id.to_string(), duration);
    }
}

/// Flush every active tag tracking, persisting durations and
/// clearing the map. Mirrors `tag-manager.js:onTimerPause` /
/// `onTimerStop` — both walk `activeSessionTags`, save the partial
/// durations, and reset the map for the next start.
pub(super) fn tag_tracking_flush_all(map: StoredValue<HashMap<String, (String, i64)>>, now: i64) {
    let drained: Vec<(String, (String, i64))> = map
        .try_update_value(|m| m.drain().collect())
        .unwrap_or_default();
    for (tag_id, (session_id, start_ms)) in drained {
        let duration = u32::try_from(((now - start_ms) / 1000).max(0)).unwrap_or(0);
        save_session_tag(session_id, tag_id, duration);
    }
}

/// Dispatch the per-tag time-spent side-effects for each engine event.
pub(super) fn apply_tag_tracking_events(
    events: &[TimerEvent],
    map: StoredValue<HashMap<String, (String, i64)>>,
    selected_tag_ids: RwSignal<Vec<String>>,
) {
    if events.is_empty() {
        return;
    }
    let now = BrowserClock.now_ms();
    for ev in events {
        match tag_tracking_action_for_event(ev) {
            TagTrackingAction::StartAllSelected => {
                let session_id = format!("session-{now}");
                let ids = selected_tag_ids.get_untracked();
                for id in &ids {
                    tag_tracking_start(map, id, &session_id, now);
                }
            }
            TagTrackingAction::FlushAll => {
                tag_tracking_flush_all(map, now);
            }
            TagTrackingAction::NoOp => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{tag_tracking_action_for_event, TagTrackingAction};
    use crate::bridge::types::TimerMode;
    use crate::engine::timer::TimerEvent;

    #[test]
    fn starts_on_session_started() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::SessionStarted),
            TagTrackingAction::StartAllSelected,
        );
    }

    #[test]
    fn starts_on_session_resumed() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::SessionResumed),
            TagTrackingAction::StartAllSelected,
        );
    }

    #[test]
    fn starts_on_auto_resumed() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::AutoResumed),
            TagTrackingAction::StartAllSelected,
        );
    }

    #[test]
    fn flushes_on_session_paused() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::SessionPaused),
            TagTrackingAction::FlushAll,
        );
    }

    #[test]
    fn flushes_on_auto_paused() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::AutoPaused),
            TagTrackingAction::FlushAll,
        );
    }

    #[test]
    fn flushes_on_pomodoro_completed() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::PomodoroCompleted {
                completed_pomodoros: 1
            }),
            TagTrackingAction::FlushAll,
        );
    }

    #[test]
    fn flushes_on_break_completed() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::BreakCompleted {
                mode: TimerMode::Break,
            }),
            TagTrackingAction::FlushAll,
            "BreakCompleted MUST flush trackers like other completion events",
        );
    }

    #[test]
    fn flushes_on_session_skipped() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::SessionSkipped {
                skipped_mode: TimerMode::Focus,
                elapsed_secs: 0,
            }),
            TagTrackingAction::FlushAll,
        );
    }

    #[test]
    fn no_op_on_overtime_started() {
        // Overtime must NOT flush — the active session is still
        // running, just past zero. Flushing would zero out per-tag
        // durations mid-session.
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::OvertimeStarted {
                mode: TimerMode::Focus,
            }),
            TagTrackingAction::NoOp,
        );
    }

    #[test]
    fn no_op_on_two_minutes_warning() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::TwoMinutesRemaining),
            TagTrackingAction::NoOp,
        );
    }

    #[test]
    fn no_op_on_thirty_seconds_warning() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::ThirtySecondsRemaining),
            TagTrackingAction::NoOp,
        );
    }

    #[test]
    fn no_op_on_manual_session_recorded() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::ManualSessionRecorded { duration_secs: 60 }),
            TagTrackingAction::NoOp,
        );
    }

    #[test]
    fn flushes_on_session_aborted() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::SessionAborted {
                aborted_mode: TimerMode::Focus,
                elapsed_secs: 0,
            }),
            TagTrackingAction::FlushAll,
        );
    }

    #[test]
    fn no_op_on_session_completed_early() {
        assert_eq!(
            tag_tracking_action_for_event(&TimerEvent::SessionCompletedEarly { elapsed_secs: 30 }),
            TagTrackingAction::NoOp,
        );
    }
}
