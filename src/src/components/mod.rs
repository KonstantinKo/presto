// Components — Phase 4 of spec 001-leptos-migration.
//
// Each component is a Leptos 0.7 `#[component]` function returning
// `impl IntoView`. Components READ engine + manager state via signals
// (or context-provided structs) and DISPATCH user actions (button
// clicks, form input) into the bridge command surface; per
// Principle I they never mutate engine state directly except through
// the documented manager APIs.
//
// Per Principle V (Test-First For Stateful Engines), UI plumbing is
// out of test-first scope — coverage is via the e2e suite + visual
// regression suite. Each component lands as a single GREEN commit
// without a paired RED test (see AGENTS.md §"Test-first commit
// ordering": "UI plumbing and trivial CRUD are out of Principle V
// scope and don't need this ordering").
//
// Selector contract: every `id="..."` and class used as an e2e test
// selector in `tests/e2e/*.spec.js` is preserved. Drift here breaks
// the e2e suite; the tests in this module pin the selector strings
// inline.

pub mod browser_clock;
pub mod calendar;
pub mod daily;
pub mod icon;
pub mod settings;
pub mod tasks;
pub mod timer;
pub mod update_notification;
pub mod utils;
