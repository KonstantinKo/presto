// Per-period tag-usage pie + legend.
//
// Static-only in v1 per FR-050 / CHK042 — hovering or clicking a slice
// does NOT highlight, filter, or otherwise modify the bar chart above.
// Cross-filtering carries its own UX surface (keyboard nav, screen-
// reader announcement of filter state, "clear filter" affordance) that
// is deferred to a follow-up spec.
//
// The pie is rendered as a CSS conic-gradient so the slice geometry is
// pure presentation (no SVG runtime + no chart library); the legend
// below uses the same `IconClass`-driven render path as the tag picker
// so emoji-only legacy tags still render via the `Glyph` fallback
// (FR-024).

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion (pie SVG + legend list). Matches `calendar.rs:32` precedent."
)]

use leptos::prelude::*;
use leptos_i18n::{t, t_string};

use crate::bridge::types::{ManualSession, Tag};
use crate::components::icon::{self, IconClass};
use crate::i18n::i18n::use_i18n;

/// One aggregated tag-usage entry — the materialised input to the
/// legend list and the pie's conic-gradient stop sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagUsageEntry {
    pub tag_id: String,
    pub tag_name: String,
    pub tag_color: String,
    pub tag_icon: String,
    /// Total focus-minutes attributed to this tag across the period's
    /// session set.
    pub minutes: u32,
}

/// Sum focus-minutes per tag across the period's session set. Tags
/// not referenced by any session are excluded from the result. Stable
/// sort by `tag_name` so the legend is alphabetised.
#[must_use]
pub fn aggregate_tag_usage(sessions: &[ManualSession], tags: &[Tag]) -> Vec<TagUsageEntry> {
    let mut totals: Vec<(String, u32)> = Vec::new();
    for session in sessions {
        let Some(session_tags) = session.tags.as_deref() else {
            continue;
        };
        for tag_value in session_tags {
            let Some(tag_id) = tag_value.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if let Some(entry) = totals.iter_mut().find(|(id, _)| id == tag_id) {
                entry.1 = entry.1.saturating_add(session.duration);
            } else {
                totals.push((tag_id.to_string(), session.duration));
            }
        }
    }

    let mut entries: Vec<TagUsageEntry> = totals
        .into_iter()
        .filter_map(|(tag_id, minutes)| {
            tags.iter().find(|t| t.id == tag_id).map(|t| TagUsageEntry {
                tag_id: t.id.clone(),
                tag_name: t.name.clone(),
                tag_color: t.color.clone(),
                tag_icon: t.icon.clone(),
                minutes,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.tag_name.cmp(&b.tag_name));
    entries
}

/// Build the CSS conic-gradient style value for the given entries.
/// Returns `None` when the entries are empty so the caller can render
/// the empty-state pill instead.
#[must_use]
pub fn conic_gradient_style(entries: &[TagUsageEntry]) -> Option<String> {
    let total: u32 = entries.iter().map(|e| e.minutes).sum();
    if total == 0 {
        return None;
    }
    let mut acc: f64 = 0.0;
    let mut stops: Vec<String> = Vec::with_capacity(entries.len());
    for entry in entries {
        let fraction = f64::from(entry.minutes) / f64::from(total);
        let next = fraction.mul_add(100.0, acc);
        stops.push(format!(
            "{color} {start:.4}% {end:.4}%",
            color = entry.tag_color,
            start = acc,
            end = next,
        ));
        acc = next;
    }
    Some(format!("background: conic-gradient({});", stops.join(", ")))
}

/// Per-period tag-usage pie + legend. Reads the period-scoped session
/// slice from the caller (already filtered to the active cursor's
/// span) and the global `tags` context. Static-only per FR-050.
#[component]
pub fn TagUsagePie(
    sessions: Signal<Vec<ManualSession>>,
    tags: Signal<Vec<Tag>>,
    #[prop(into, optional)] title: Signal<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let entries =
        Signal::derive(move || sessions.with(|ss| tags.with(|ts| aggregate_tag_usage(ss, ts))));
    let resolved_title = Signal::derive(move || {
        let raw = title.get();
        if raw.is_empty() {
            t_string!(i18n, stats.tag_usage_default_title).to_string()
        } else {
            raw
        }
    });

    view! {
        <div class="tag-usage-card">
            <h3>{move || resolved_title.get()}</h3>
            {move || {
                let mut snapshot = entries.get();
                // The persisted seed for the `default-focus` tag carries a
                // green hex (#4CAF50) inherited from the JS-era data —
                // semantically misleading here, where the pie shows focus
                // time and green is the break colour. Swap to the live
                // `--focus-color` token so the segment + swatch follow the
                // active theme's focus tint.
                for entry in &mut snapshot {
                    if entry.tag_id == "default-focus" && entry.tag_color.eq_ignore_ascii_case("#4CAF50") {
                        entry.tag_color = "var(--focus-color)".to_string();
                    }
                }
                if snapshot.is_empty() {
                    view! {
                        <div class="tag-usage-pie-row">
                            <div class="tag-usage-pie tag-usage-pie-empty"></div>
                            <div class="tag-usage-empty">
                                {t!(i18n, stats.tag_usage_empty)}
                            </div>
                        </div>
                    }.into_any()
                } else {
                    let pie_style = conic_gradient_style(&snapshot).unwrap_or_default();
                    let legend_items = snapshot.into_iter().map(|entry| {
                        let icon_class = IconClass::from_icon_name(&entry.tag_icon);
                        let swatch_style = format!("background: {};", entry.tag_color);
                        let minutes_text = format!("{} min", entry.minutes);
                        let display_name = if entry.tag_id == "default-focus" && entry.tag_name == "Focus" {
                            t_string!(i18n, tag.default_name).to_string()
                        } else {
                            entry.tag_name
                        };
                        view! {
                            <li class="tag-usage-legend-item">
                                <span class="tag-usage-swatch" style=swatch_style></span>
                                {icon::render(&icon_class)}
                                <span class="tag-usage-name">{display_name}</span>
                                <span class="tag-usage-minutes">{minutes_text}</span>
                            </li>
                        }
                    }).collect_view();
                    view! {
                        <div class="tag-usage-pie-row">
                            <div class="tag-usage-pie" style=pie_style></div>
                            <ul class="tag-usage-legend">
                                {legend_items}
                            </ul>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{aggregate_tag_usage, conic_gradient_style};
    use crate::bridge::types::{ManualSession, SessionType, Tag};
    use serde_json::json;

    fn tag(id: &str, name: &str, color: &str) -> Tag {
        Tag {
            id: id.to_string(),
            name: name.to_string(),
            icon: "ri-brain-line".to_string(),
            color: color.to_string(),
            created_at: String::new(),
        }
    }

    fn session(date: &str, duration: u32, tag_ids: &[&str]) -> ManualSession {
        let tags_payload: Vec<serde_json::Value> = tag_ids
            .iter()
            .map(|id| json!({ "id": id, "name": id }))
            .collect();
        ManualSession {
            id: format!("s-{date}-{duration}"),
            session_type: SessionType::Focus,
            duration,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: format!("{date}T09:00:00Z"),
            date: date.to_string(),
            tags: Some(tags_payload),
            title: None,
        }
    }

    #[test]
    fn aggregate_empty_when_no_sessions() {
        let tags = vec![tag("focus", "Focus", "#4CAF50")];
        let entries = aggregate_tag_usage(&[], &tags);
        assert!(entries.is_empty());
    }

    #[test]
    fn aggregate_sums_per_tag_and_alphabetises() {
        let tags = vec![
            tag("focus", "Focus", "#4CAF50"),
            tag("admin", "Admin", "#2196F3"),
        ];
        let sessions = vec![
            session("Mon May 4 2026", 25, &["focus"]),
            session("Tue May 5 2026", 30, &["focus"]),
            session("Wed May 6 2026", 15, &["admin"]),
        ];
        let entries = aggregate_tag_usage(&sessions, &tags);
        assert_eq!(entries.len(), 2);
        // Alphabetised by tag_name; Admin first.
        assert_eq!(entries[0].tag_name, "Admin");
        assert_eq!(entries[0].minutes, 15);
        assert_eq!(entries[1].tag_name, "Focus");
        assert_eq!(entries[1].minutes, 55);
    }

    #[test]
    fn aggregate_skips_sessions_with_no_tags() {
        let tags = vec![tag("focus", "Focus", "#4CAF50")];
        let mut session = session("Mon May 4 2026", 25, &["focus"]);
        session.tags = None;
        let entries = aggregate_tag_usage(&[session], &tags);
        assert!(entries.is_empty());
    }

    #[test]
    fn conic_gradient_none_for_empty_entries() {
        assert_eq!(conic_gradient_style(&[]), None);
    }
}
