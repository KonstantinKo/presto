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

#[cfg(test)]
mod tests {
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
    }
}
