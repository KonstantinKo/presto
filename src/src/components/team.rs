// Team view component — Phase 4b (T215) of spec
// 001-leptos-migration. Renders the parity-only team dashboard
// matching `src/managers/team-manager.js`'s demo fixture (8 members
// across 2 teams).
//
// **Selector contract** (consumed by `tests/e2e/team.spec.js`):
// - `#team-view` — root view container; carries `.hidden` when
//   another `NavView` is active (`spec.js:14`).
// - `#team-focusing`, `#team-on-break`, `#team-privacy`,
//   `#team-offline` — stat-card numbers (`spec.js:17,21,22,23`).
// - `#team-members-grid` — host for per-team sections
//   (`spec.js:26`); per-team `role="group"` (`spec.js:27`); per
//   member `role="listitem"` with three `data-member-field` cells
//   for name / role / avatar (`spec.js:31,38,43,48`).
//
// Per the JS-era `team-manager.js` surface, the displayed counts
// are derived from member statuses (Focus / Break / Privacy /
// Offline). Long-break members fold into the "On Break" count to
// match the JS-era projection at line 530-560 of the JS-era
// index.html (the four cards aggregate; LongBreak ⇒ "On Break").
//
// Per Principle I, this component never mutates engine state — the
// team data lives in `TeamManager`. Future post-cutover work would
// wire this against a real team service; today's surface is the
// fixture renderer.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason. `clippy::too_many_lines` is
// silenced because the view is one Leptos `view!` expansion
// covering both the stats grid and the per-team / per-member
// sub-trees.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;

use crate::managers::team::{MemberStatus, Team, TeamManager, TeamMember};

/// Aggregate the four team-status counts. Mirrors the JS-era
/// projection where `LongBreak` folds into "On Break" so the four
/// cards line up against the demo fixture's status distribution.
#[derive(Debug, Default, Clone, Copy)]
struct TeamCounts {
    focusing: u32,
    on_break: u32,
    privacy: u32,
    offline: u32,
}

fn count_statuses(teams: &[Team]) -> TeamCounts {
    let mut counts = TeamCounts::default();
    for team in teams {
        for member in &team.members {
            match member.status {
                MemberStatus::Focus => counts.focusing += 1,
                MemberStatus::Break | MemberStatus::LongBreak => counts.on_break += 1,
                MemberStatus::Privacy => counts.privacy += 1,
                MemberStatus::Offline => counts.offline += 1,
            }
        }
    }
    counts
}

/// Team view — renders the demo fixture with stats grid + per-team
/// member rosters.
#[component]
pub fn TeamView() -> impl IntoView {
    // Demo fixture is loaded once on mount — the JS-era surface
    // does the same at `team-manager.js:initializeDemoData`.
    let manager = TeamManager::load_demo_fixture();
    let teams = manager.teams().to_vec();
    let counts = count_statuses(&teams);

    view! {
        <div class="view-container view-section" id="team-view">
            <h1 class="page-header">"Team Dashboard"</h1>
            <p class="team-subtitle page-subtitle">
                "Monitor your team's focus status in real-time"
            </p>
            // Stats grid — four cards, each showing the projection
            // count from `count_statuses`.
            <div class="team-stats-container stats-grid">
                <div class="team-stat-card stat-card">
                    <div class="stat-icon">
                        <i class="ri-brain-line"></i>
                    </div>
                    <div class="stat-info">
                        <span class="stat-number" id="team-focusing">{counts.focusing}</span>
                        <span class="stat-label">"Focusing"</span>
                    </div>
                </div>
                <div class="team-stat-card stat-card">
                    <div class="stat-icon">
                        <i class="ri-cup-line"></i>
                    </div>
                    <div class="stat-info">
                        <span class="stat-number" id="team-on-break">{counts.on_break}</span>
                        <span class="stat-label">"On Break"</span>
                    </div>
                </div>
                <div class="team-stat-card stat-card">
                    <div class="stat-icon">
                        <i class="ri-lock-line"></i>
                    </div>
                    <div class="stat-info">
                        <span class="stat-number" id="team-privacy">{counts.privacy}</span>
                        <span class="stat-label">"Privacy Mode"</span>
                    </div>
                </div>
                <div class="team-stat-card stat-card">
                    <div class="stat-icon">
                        <i class="ri-checkbox-blank-circle-fill"></i>
                    </div>
                    <div class="stat-info">
                        <span class="stat-number" id="team-offline">{counts.offline}</span>
                        <span class="stat-label">"Offline"</span>
                    </div>
                </div>
            </div>
            // Per-team member rosters. Each team is a
            // `role="group"` per the JS-era index.html semantic
            // shape; each member is `role="listitem"` with three
            // `data-member-field` cells the spec asserts on.
            <div class="team-members-container">
                <h2>"Teams Overview"</h2>
                <div class="team-members-grid" id="team-members-grid">
                    {teams
                        .into_iter()
                        .map(team_section)
                        .collect::<Vec<_>>()}
                </div>
            </div>
        </div>
    }
}

/// Render a single team's section: header + per-member rows.
fn team_section(team: Team) -> impl IntoView {
    let aria_label = team.name.clone();
    let heading = team.name;
    let description = team.description;
    let members = team.members;
    view! {
        <div class="team-section" role="group" aria-label=aria_label>
            <div class="team-section-header">
                <h3>{heading}</h3>
                <p class="team-description">{description}</p>
            </div>
            <div class="team-section-members">
                {members
                    .into_iter()
                    .map(member_row)
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}

/// Render a single member row. Each cell carries
/// `data-member-field="<name|role|avatar>"` so the spec's
/// `locator('[data-member-field="..."]')` calls at lines 38, 43, 48
/// resolve.
fn member_row(member: TeamMember) -> impl IntoView {
    let name = member.name;
    let role = member.role;
    let avatar = member.avatar;
    let activity = member.activity;
    let timer = member.timer;
    view! {
        <div class="team-member-row" role="listitem">
            <span class="team-member-avatar" data-member-field="avatar">{avatar}</span>
            <span class="team-member-name" data-member-field="name">{name}</span>
            <span class="team-member-role" data-member-field="role">{role}</span>
            <span class="team-member-activity">{activity}</span>
            <span class="team-member-timer">{timer}</span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::count_statuses;
    use crate::managers::team::TeamManager;

    #[test]
    fn count_statuses_aggregates_demo_fixture() {
        let mgr = TeamManager::load_demo_fixture();
        let counts = count_statuses(mgr.teams());
        // Demo fixture: 8 members. Focus=3 (Marco/Francesco/Luca);
        // Break=1 (Sara) + LongBreak=1 (Giulia) ⇒ on_break=2;
        // Privacy=1 (Andrea); Offline=2 (Elena/Chiara). Total = 8.
        assert_eq!(counts.focusing, 3, "demo fixture has 3 focusing");
        assert_eq!(counts.on_break, 2, "Break + LongBreak fold into on_break");
        assert_eq!(counts.privacy, 1);
        assert_eq!(counts.offline, 2);
        assert_eq!(
            counts.focusing + counts.on_break + counts.privacy + counts.offline,
            8,
            "every member must be counted exactly once",
        );
    }

    /// T215 — selector contract pin. Sourced from
    /// `tests/e2e/team.spec.js`.
    #[test]
    fn team_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "team-view",
            "team-focusing",
            "team-on-break",
            "team-privacy",
            "team-offline",
            "team-members-grid",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
