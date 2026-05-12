# Research: Feature 003 — Statistics Redesign + Phosphor Icons + Tooltips

**Branch**: `003-stats-redesign` | **Date**: 2026-05-12

The spec.md Clarifications-resolved log already documents the seven CHK / 2026-05-12 decisions (vendored Phosphor font, decoupled `aria-label` vs `data-tooltip` per WCAG 4.1.2, e2e selector preservation policy, new sidebar Daily entry, sidebar `box-shadow` vs `border-right` constraint, four per-period visual-regression baselines per CHK040, static-only tag-usage pie per CHK042, and the `#sessions-table-body` migration from Statistics to Daily per CHK043). This file records the **one irreversible external decision** that justifies a separate research note rather than rehashing the spec.

## Decision: Vendor the Phosphor regular-weight webfont via Trunk `copy-dir` (no CDN, no `phosphor-react`)

### Decision

The Phosphor icons used by Bundle C ship as a vendored asset bundle under `src/assets/icons/phosphor/`, copied into the Trunk dist tree via the existing `data-trunk rel="copy-dir"` mechanism (mirroring the remixicon block at `src/index.html:19-27`). The vendoring source is `@phosphor-icons/web` v2.x, added to `tests/e2e/package.json` as a `devDependency` — the runtime bundle does not link against it; only the webfont files and the CSS file are committed to the repo. Only the **regular** weight is vendored (~50 KB); thin / light / bold / fill / duotone weights are not included.

### Rationale

Three forces converge on this decision:

1. **The e2e suite blocks remote font fetches.** `tests/e2e/fixtures/_blockExternal` blocks all non-loopback HTTP (per `tests/e2e/CLAUDE.md` Rule 2: "All non-loopback HTTP requests are blocked by the `_blockExternal` auto-fixture. No test may override this to reach the internet."). A CDN link in `src/index.html` would fail to load the font during e2e runs, producing baseline drift (the icons would render as empty `<i>` elements or fall back to the system font's `?` glyph). The PM brief's mention of "CDN link in `index.html`" was rejected here as a verified-fact correction over the brief (spec A7).
2. **Same-origin parity with remixicon.** The remixicon webfont is already vendored via `<link data-trunk rel="copy-dir" href="assets/icons" data-target-path="assets/icons" />` + `<link rel="stylesheet" href="/assets/icons/remixicon.css" />` at `src/index.html:19-27`. Phosphor adopts the same shape — `assets/icons/phosphor/` (subdirectory under the existing `assets/icons/` root) with its own `phosphor.css`. Trunk's `copy-dir` is a verbatim-copy directive; it preserves the `@font-face url(...)` relative paths in the CSS without any post-processing.
3. **Bundle size + Principle IX (Lock Files Are First-Class).** Full Phosphor (all weights, duotone, fill) is ~250 KB. The nine glyphs the picker uses (`ph-butterfly`, `ph-cloud`, `ph-code-simple`, `ph-github-logo`, `ph-apple-logo`, `ph-crown-simple`, `ph-atom`, `ph-student`, `ph-cpu`) plus the two sidebar glyphs (`ph-chart-line-up`, `ph-calendar-check`) all render correctly in regular weight (verified against Phosphor's webfont showcase). Shipping regular-only saves ~200 KB of asset weight in the install bundle, on a tool where the entire WASM payload is in the same order of magnitude. The `@phosphor-icons/web` `devDependency` is locked via `tests/e2e/package-lock.json` so any contributor running the vendoring script (per `quickstart.md`) gets the same font files; this is the lockfile rule's documented mechanism (Principle IX).

### Alternatives considered

- **CDN link** (`<link href="https://unpkg.com/@phosphor-icons/web@2/.../phosphor.css">`): rejected because the e2e suite's `_blockExternal` fixture would block it, producing baseline drift. Even if the fixture were overridden, the auto-updater pings already document the only outbound traffic Presto makes; adding CDN traffic violates Principle II (Local-Only) for non-user-data egress in spirit (no user data on the wire, but a network dependency added to every cold-load).
- **`phosphor-react` (or any Rust crate wrapping Phosphor)**: rejected because the npm package is a React component library — not usable from Leptos/Rust without a JS interop bridge — and the typical Cargo wrappers ship SVGs inline, which would inflate the WASM payload more than the webfont alternative. The webfont approach exactly parallels remixicon, which is the project's existing canonical icon-vendoring path.
- **Inline SVG copy-paste for the 11 glyphs**: rejected because Bundle C's typed-dispatch renderer (`IconClass::Phosphor`) expects to emit `<i class="ph ph-X">` and rely on the font's CSS for the glyph mapping — inlining SVGs would require a separate `match` over the 11 specific glyphs at render time, defeating the closed-sum-type-with-string-payload design of FR-023. (The Glyph variant covers raw graphemes, not SVG strings.)
- **All Phosphor weights**: rejected per A8. The 5x weight surface area is unused by this feature and a follow-up that needs thin/duotone can add the weights then.

### Vendoring procedure (operational)

The exact contributor procedure is documented in `quickstart.md`. Summary: `npm install --save-dev @phosphor-icons/web` in `tests/e2e/`; copy the regular-weight font files from `node_modules/@phosphor-icons/web/src/regular/` to `src/assets/icons/phosphor/`; rename `style.css` → `phosphor.css`; commit the assets + the regenerated `package-lock.json` in the same commit (Principle IX). The committed assets are the canonical artefacts; the `devDependency` only exists so re-vendoring from a new Phosphor release is reproducible.
