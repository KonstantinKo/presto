// Engine — chrono format pin for `Session.date`.
//
// Spec 001-leptos-migration §Phase 2 T144-T145; data-model.md
// §`Session.date`. Pins the chrono format string `"%a %b %d %Y"`
// against JS `Date.prototype.toDateString()` parity so a future
// chrono change that breaks parity fails loud at CI time rather
// than silently corrupting on-disk session dates.
