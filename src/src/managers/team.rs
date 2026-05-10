// `TeamManager` — the Rust port of `src/managers/team-manager.js`.
//
// Spec 001-leptos-migration §Phase 3c (T187-T188). The JS-era
// surface is parity-only — a hardcoded demo fixture (2 teams, 8
// members) plus a `setInterval`-driven mutation loop that randomly
// flips member statuses every 30s. The Rust port preserves the
// fixture shape so the components layer (Phase 4) can render the
// Team view; the random-mutation loop is a UI concern and lives in
// components, not here.
//
// No bridge calls — the team data is purely demo / display state.
// Future post-cutover work (out of Phase 3c scope) would wire this
// against a real team service; today's surface is the fixture
// renderer.

/// Per-member status.
///
/// Mirrors the JS-era `member.status` strings at
/// `src/managers/team-manager.js:77, 89, 101, 113, …` — closed
/// sum type per FR-013, so a future drift to a new status string
/// breaks compilation rather than silently rendering as the unknown
/// fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStatus {
    /// In a focus session.
    Focus,
    /// In a short break.
    Break,
    /// In a long break.
    LongBreak,
    /// Privacy mode (timer hidden, activity reads "Privacy mode
    /// enabled").
    Privacy,
    /// Offline / last-seen-some-time-ago.
    Offline,
}

/// One team member's display row.
///
/// Mirrors the JS-era `member` object shape at
/// `src/managers/team-manager.js:73-83` minus the `lastSeen` /
/// `currentSessionStart` fields (those are simulation-state for the
/// random-mutation loop, which is a UI concern that lives in Phase
/// 4 components).
#[derive(Debug, Clone)]
pub struct TeamMember {
    /// Stable per-member id (1..=8 in the demo fixture).
    pub id: u32,
    /// Display name ("Marco Rossi").
    pub name: String,
    /// Role caption ("Frontend Developer").
    pub role: String,
    /// Two-letter initials avatar ("MR").
    pub avatar: String,
    /// Current status — closed-domain enum.
    pub status: MemberStatus,
    /// Pre-formatted timer string ("18:35"). The JS-era `tickDemoData`
    /// loop re-computes this every 30s; the Rust port treats it as
    /// static fixture data because the loop lives in Phase 4.
    pub timer: String,
    /// Activity caption ("Working on React components").
    pub activity: String,
    /// Today's accumulated focus minutes — display-only stat.
    pub total_focus_today: u32,
}

/// One team's roster. Mirrors the JS-era team object shape at
/// `src/managers/team-manager.js:67-71`.
#[derive(Debug, Clone)]
pub struct Team {
    /// Stable team id (1 or 2 in the demo fixture).
    pub id: u32,
    /// Display name ("Team Frontend").
    pub name: String,
    /// Description caption ("React & Vue.js Development").
    pub description: String,
    /// Members in this team. The JS-era surface is a flat array per
    /// team; the Rust port mirrors that.
    pub members: Vec<TeamMember>,
}

/// Team-roster manager. Owns the demo fixture; no bridge calls.
///
/// Phase 3c lands `load_demo_fixture` (T188); future work
/// post-cutover would attach a real team service. The fixture
/// matches the JS-era shape at `team-manager.js:65-178` byte-for-byte
/// on the per-member fields the Rust port carries.
#[derive(Debug, Clone, Default)]
pub struct TeamManager {
    teams: Vec<Team>,
}

impl TeamManager {
    /// Construct an empty manager. The demo fixture loader is the
    /// canonical entry point; this constructor is the cold-start
    /// "no fixture loaded" shape.
    #[must_use]
    pub const fn new() -> Self {
        Self { teams: Vec::new() }
    }

    /// Borrow the current team list.
    #[must_use]
    pub fn teams(&self) -> &[Team] {
        &self.teams
    }

    /// Load the JS-era demo fixture. Mirrors
    /// `src/managers/team-manager.js:65-178` `initializeDemoData`,
    /// minus the simulation-state fields (`lastSeen`,
    /// `currentSessionStart`) which are owned by the components
    /// layer's mutation loop.
    ///
    /// Spec 001-leptos-migration §Phase 3c T188.
    #[must_use]
    pub fn load_demo_fixture() -> Self {
        Self {
            teams: vec![
                Team {
                    id: 1,
                    name: "Team Frontend".to_string(),
                    description: "React & Vue.js Development".to_string(),
                    members: vec![
                        TeamMember {
                            id: 1,
                            name: "Marco Rossi".to_string(),
                            role: "Frontend Developer".to_string(),
                            avatar: "MR".to_string(),
                            status: MemberStatus::Focus,
                            timer: "18:35".to_string(),
                            activity: "Working on React components".to_string(),
                            total_focus_today: 180,
                        },
                        TeamMember {
                            id: 2,
                            name: "Sara Bianchi".to_string(),
                            role: "UX Designer".to_string(),
                            avatar: "SB".to_string(),
                            status: MemberStatus::Break,
                            timer: "3:20".to_string(),
                            activity: "Short break".to_string(),
                            total_focus_today: 125,
                        },
                        TeamMember {
                            id: 3,
                            name: "Francesco Galli".to_string(),
                            role: "Frontend Developer".to_string(),
                            avatar: "FG".to_string(),
                            status: MemberStatus::Focus,
                            timer: "22:10".to_string(),
                            activity: "Code review session".to_string(),
                            total_focus_today: 165,
                        },
                        TeamMember {
                            id: 4,
                            name: "Elena Conti".to_string(),
                            role: "UI Designer".to_string(),
                            avatar: "EC".to_string(),
                            status: MemberStatus::Offline,
                            timer: "--:--".to_string(),
                            activity: "Last seen yesterday".to_string(),
                            total_focus_today: 0,
                        },
                    ],
                },
                Team {
                    id: 2,
                    name: "Team Backend".to_string(),
                    description: "API & Database Development".to_string(),
                    members: vec![
                        TeamMember {
                            id: 5,
                            name: "Luca Verdi".to_string(),
                            role: "Backend Developer".to_string(),
                            avatar: "LV".to_string(),
                            status: MemberStatus::Focus,
                            timer: "12:45".to_string(),
                            activity: "API development".to_string(),
                            total_focus_today: 205,
                        },
                        TeamMember {
                            id: 6,
                            name: "Giulia Neri".to_string(),
                            role: "Product Manager".to_string(),
                            avatar: "GN".to_string(),
                            status: MemberStatus::LongBreak,
                            timer: "15:00".to_string(),
                            activity: "Long break".to_string(),
                            total_focus_today: 150,
                        },
                        TeamMember {
                            id: 7,
                            name: "Andrea Ferrari".to_string(),
                            role: "DevOps Engineer".to_string(),
                            avatar: "AF".to_string(),
                            status: MemberStatus::Privacy,
                            timer: "--:--".to_string(),
                            activity: "Privacy mode enabled".to_string(),
                            total_focus_today: 95,
                        },
                        TeamMember {
                            id: 8,
                            name: "Chiara Romano".to_string(),
                            role: "QA Engineer".to_string(),
                            avatar: "CR".to_string(),
                            status: MemberStatus::Offline,
                            timer: "--:--".to_string(),
                            activity: "Last seen 2 hours ago".to_string(),
                            total_focus_today: 75,
                        },
                    ],
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemberStatus;

    /// T187 [RED]: `TeamManager::load_demo_fixture()` returns the
    /// JS-era demo fixture shape: 2 teams, 8 total members. Mirrors
    /// `src/managers/team-manager.js:65-178` `initializeDemoData`.
    ///
    /// Done-signal: this test currently fails because
    /// `TeamManager::load_demo_fixture` does not yet exist. T188
    /// GREEN attaches it alongside the `Team` / `TeamMember` /
    /// `MemberStatus` data types.
    #[test]
    fn demo_fixture_loads() {
        let mgr = super::TeamManager::load_demo_fixture();
        assert_eq!(mgr.teams().len(), 2, "JS-era demo fixture has 2 teams");
        let total_members: usize = mgr.teams().iter().map(|t| t.members.len()).sum();
        assert_eq!(total_members, 8, "JS-era demo fixture has 8 total members");
        // Pin team names for parity — `team-manager.js:69, 124`.
        assert_eq!(mgr.teams()[0].name, "Team Frontend");
        assert_eq!(mgr.teams()[1].name, "Team Backend");
        // Spot-check a member to pin the per-row shape.
        let marco = &mgr.teams()[0].members[0];
        assert_eq!(marco.name, "Marco Rossi");
        assert_eq!(marco.avatar, "MR");
        assert_eq!(marco.status, MemberStatus::Focus);
        assert_eq!(marco.total_focus_today, 180);
    }
}
