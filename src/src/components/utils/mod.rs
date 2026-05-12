// Shared, non-view helpers used by multiple component modules.
//
// Promoted in feature 003 (T003): `datetime_from_ms` was inlined in
// `components::calendar` but is needed by both `components::stats`
// (Bundle A) and `components::daily` (Bundle B). Living under
// `components::utils` (rather than at crate root) keeps the helper
// adjacent to its consumers — every caller is a view module.

pub mod datetime;
