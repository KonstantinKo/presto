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
// Phase 4a (T189-T203): the five core screens — Timer, Tasks,
// History, Calendar, Tags. Phase 4b layers on Settings tabs, the
// Auth modal, the update-notification banner, and Team. Phase 4c
// wires the top-level `App` router that dispatches `NavView` over
// these.
//
// Selector contract: every `id="..."` and class used as an e2e test
// selector in `tests/e2e/*.spec.js` is preserved post-cutover. Drift
// here breaks the e2e suite (Phase 6 gate); the tests in this module
// pin the selector strings inline.

// Phase 4a lands the modules incrementally — each task adds one
// `pub mod` declaration alongside its skeleton file. Timer is the
// first (T189); Tasks (T192), History (T195), Calendar (T198) and
// Tags (T201) follow. Phase 4b extends with the settings shell + 8
// tabs (T204-T212), the auth modal (T213), the update banner
// (T214), the team panel (T215), and the App router (T216-T218).
pub mod auth_modal;
pub mod calendar;
pub mod history;
pub mod settings;
pub mod tags;
pub mod tasks;
pub mod timer;
