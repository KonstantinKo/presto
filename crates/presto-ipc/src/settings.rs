// `Settings` (+ nested + `SettingsOnDisk` legacy-shape shim) — the
// canonical wire/on-disk record for all user preferences.
//
// Spec 001-leptos-migration §Phase 3a T150-T152;
// data-model.md §"Settings legacy migration".
//
// The Settings tree is the largest cross-crate type and was
// previously duplicated byte-for-byte between the Tauri backend and
// the Leptos frontend. Single-sourcing here closes a documented
// drift hazard (the JS-era `hide_status_bar → status_bar_display`
// migration had to be ported twice — once per crate).

use serde::{Deserialize, Serialize};

/// Status-bar visibility mode.
///
/// Replaces the legacy `hide_status_bar: bool` shape with a typed
/// enum so future "compact" or "hidden" modes don't fork the on-disk
/// encoding.
///
/// Wire shape: kebab-case strings (`"default"`, `"icon-only"`),
/// matching the JS-era on-disk values written by
/// `src/managers/settings-manager.js` after its `hide_status_bar →
/// status_bar_display` migration step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum StatusBarDisplay {
    /// Full status bar (timer text + icon).
    #[default]
    Default,
    /// Icon-only status bar — corresponds to the legacy
    /// `hide_status_bar: true` setting.
    IconOnly,
}

/// Ambient-sound track selection (feature 004).
///
/// Closed sum type: eleven variants, one per vendored ambient track
/// plus `None` ("no track selected"). `None` is a first-class
/// variant — not `Option<AmbientSoundType>` and not a string sentinel
/// — so the type system encodes the absence case directly
/// (Principle III). Wire shape is kebab-case strings (`"none"`,
/// `"rain"`, ..., `"white-noise"`, `"wind"`, `"pink-noise"`,
/// `"brown-noise"`, `"binaural"`), mirroring the `StatusBarDisplay`
/// precedent at `:27`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum AmbientSoundType {
    /// No track selected — playback is a no-op even when
    /// `ambient_sound_enabled = true`. Preserves the user's volume
    /// slider value (FR-005 / A11). Wire string `"none"`.
    #[default]
    None,
    /// Rain loop — vendored at `assets/audio/ambient/rain.wav`.
    Rain,
    /// Fire / crackle loop — vendored at `assets/audio/ambient/fire.wav`.
    Fire,
    /// Library / café ambience — vendored at
    /// `assets/audio/ambient/library.wav`.
    Library,
    /// Fan hum — vendored at `assets/audio/ambient/fan.wav`.
    Fan,
    /// Storm — vendored at `assets/audio/ambient/storm.wav`.
    Storm,
    /// White noise — vendored at
    /// `assets/audio/ambient/white-noise.wav`. CRITICAL: the
    /// multi-word variant — `#[serde(rename_all = "kebab-case")]`
    /// emits it as `"white-noise"`, not `"whitenoise"`.
    WhiteNoise,
    /// Wind — vendored at `assets/audio/ambient/wind.wav`.
    Wind,
    /// Pink noise — vendored at `assets/audio/ambient/pink-noise.wav`.
    /// Synthesised via `ffmpeg anoisesrc=color=pink`; perceptually
    /// flat across octaves and the noise-colour most cited in
    /// cognition / sleep research. Loops seamlessly because the
    /// signal is stochastic with no periodic structure.
    PinkNoise,
    /// Brown noise — vendored at
    /// `assets/audio/ambient/brown-noise.wav`. Synthesised via
    /// `ffmpeg anoisesrc=color=brown`; -6 dB/octave roll-off makes
    /// it deeper / less hissy than white. Loops seamlessly.
    BrownNoise,
    /// Binaural beat (40 Hz gamma) — vendored at
    /// `assets/audio/ambient/binaural.wav`. Pure 200 Hz / 240 Hz
    /// sines, one per channel; the 40 Hz delta is perceived as a
    /// beat only when worn on headphones. Loops seamlessly (sine
    /// periods align at any integer-second boundary).
    Binaural,
}

/// Default volume for the ambient-sound slider (feature 004).
///
/// `50` = "noticeable but not loud" per A9. Used by the
/// `#[serde(default = "default_ambient_sound_volume")]` attribute
/// on `NotificationSettings::ambient_sound_volume` so pre-feature-004
/// settings JSONs lacking the field deserialise to this value.
#[must_use]
pub const fn default_ambient_sound_volume() -> u32 {
    50
}

/// Keyboard-shortcut bindings bundle.
///
/// Each field is `Option<String>` because users can clear a binding
/// (the JS era stores `null` for cleared bindings, which serde maps
/// to `None`). Each string is a Tauri shortcut spec like
/// `"CommandOrControl+Alt+Space"`; parsing happens Rust-side at
/// `register_global_shortcuts` time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ShortcutSettings {
    pub start_stop: Option<String>,
    pub reset: Option<String>,
    pub skip: Option<String>,
    /// Feature 007. Optional binding for the Abort action (used as a
    /// keyboard-accessible discard during overtime; usable from any
    /// running state). `None` = unbound. Default is `None`.
    ///
    /// Backwards-compatible at the JSON layer: pre-feature-007
    /// settings.json files lack this key and deserialise to `None`
    /// via `serde`'s `Option<T>` missing-field default — mirroring
    /// the precedent of the three sibling fields above.
    #[serde(default)]
    pub abort: Option<String>,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: Some("CommandOrControl+Alt+R".to_string()),
            skip: Some("CommandOrControl+Alt+S".to_string()),
            // Intentional asymmetry (FR-019): `abort` defaults to `None`
            // (unbound) while the three sibling fields above are
            // pre-bound as a convenience for users who never open
            // Settings. Abort is opt-in — it is primarily a power-user
            // escape hatch during overtime. The user binds it from
            // Settings > Shortcuts when they want a keyboard discard
            // path. Do NOT "fix" this asymmetry without a spec revision.
            abort: None,
        }
    }
}

/// User-selectable UI locale (feature 005).
///
/// Closed four-variant sum type. Wire shape is lowercase strings
/// (`"en"` / `"de"` / `"it"` / `"tr"`) matching the existing `theme`
/// field's lowercase convention at `:121-123` rather than the
/// `AmbientSoundType` kebab-case precedent — two-letter ISO-639-1
/// codes have no internal word boundary that kebab-case would clarify.
///
/// The `#[default]` attribute on `En` ties this enum to
/// `#[derive(Default)]`; the default value is used by `Locale::default()`
/// callers (e.g. the resolver's terminal fallback) — NOT by the
/// `AppearanceSettings.locale` field, which uses `Option<Locale>` so
/// `None` (no explicit choice) and `Some(Locale::En)` (explicit
/// English) are distinguishable per Fix A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// English — wire string `"en"`. Source-of-truth locale per Spec A13.
    #[default]
    En,
    /// Deutsch — wire string `"de"`.
    De,
    /// Italiano — wire string `"it"`.
    It,
    /// Türkçe — wire string `"tr"`.
    Tr,
}

/// Appearance / theme preferences.
///
/// `theme` is the color-mode preference (`"auto"` / `"light"` /
/// `"dark"`); `timer_theme` is the timer palette stem (e.g.
/// `"espresso"`). Both carry `#[serde(default)]` so pre-widening
/// settings JSONs fill in the JS-era cold-start values.
///
/// `locale` (feature 005) is `Option<Locale>` — `None` = "user has
/// never explicitly chosen a locale" (legacy records or fresh install
/// — the resolver runs OS detection on cold start); `Some(Locale)` =
/// "user explicitly chose this locale" (including English — bypasses
/// OS detection per FR-011 / Fix A). The `Option` discriminant is the
/// authoritative "explicit vs. default" signal; value-equality against
/// `Locale::En` MUST NOT be used as a proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AppearanceSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_timer_theme")]
    pub timer_theme: String,
    /// Feature 005: user-selected UI locale. `None` = no explicit
    /// choice yet; `Some(_)` = explicit choice (any variant, including
    /// `Some(En)`). The `Option` discriminant is the resolver's
    /// authoritative "explicit vs. default" signal per FR-009 / FR-011.
    ///
    /// Lenient deserialisation: an out-of-set wire value (`"fr"`,
    /// `42`, etc.) degrades silently to `None` rather than failing the
    /// whole-struct parse. Spec Story 2 AC 4 requires that an unknown
    /// locale code in a hand-edited settings.json must NOT brick the
    /// settings load — the resolver then falls back to OS detection.
    /// The `Locale` enum itself remains strict on its own
    /// deserialisation surface; only this field is forgiving (two
    /// separate contracts).
    #[serde(default, deserialize_with = "deserialize_locale_lenient")]
    pub locale: Option<Locale>,
}

/// Lenient `Option<Locale>` deserialiser for
/// `AppearanceSettings.locale` (Spec Story 2 AC 4).
///
/// Routes through `serde_json::Value` so an unknown enum value (`"fr"`)
/// returns `Ok(None)` instead of bubbling a struct-level serde error.
/// `null` and a missing field both also yield `None` (the
/// `#[serde(default)]` attribute handles the missing-field case before
/// this function is even called; this function handles the
/// present-but-invalid case).
///
/// **Accepted limitation (FR-026):** an invalid wire value (`"fr"`, `42`,
/// etc.) coerces SILENTLY to `None` rather than logging or surfacing
/// the rejection. The resolver then falls back to OS-language
/// detection per FR-009 step 2 — which lands the user in a usable
/// state without bricking the settings load. Telemetry on this branch
/// is deliberately omitted in v1 (presto is single-user, fully local,
/// no analytics surface — see VISION.md). A future surface (an
/// "advanced diagnostics" toggle) could route through `tracing` if
/// debugging hand-edited settings files becomes a recurring need.
fn deserialize_locale_lenient<'de, D>(deserializer: D) -> Result<Option<Locale>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(v) => Ok(serde_json::from_value::<Locale>(v).ok()),
    }
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            timer_theme: default_timer_theme(),
            locale: None,
        }
    }
}

fn default_theme() -> String {
    "auto".to_string()
}

fn default_timer_theme() -> String {
    "espresso".to_string()
}

/// Timer durations & session count.
///
/// `weekly_goal_minutes` and `max_session_time` carry
/// `#[serde(default = "...")]` because settings JSON written by
/// pre-widening builds lacks those fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TimerSettings {
    /// Minutes.
    pub focus_duration: u32,
    /// Minutes.
    pub break_duration: u32,
    /// Minutes.
    pub long_break_duration: u32,
    pub total_sessions: u32,
    #[serde(default = "default_weekly_goal")]
    pub weekly_goal_minutes: u32,
    /// Maximum continuous session time before auto-pause (minutes).
    #[serde(default = "default_max_session_time")]
    pub max_session_time: u32,
    /// Number of focus completions per long-break cycle (1–10
    /// enforced at the Settings UI input boundary, per Principle III).
    /// The engine reads this as a configuration input alongside
    /// `Durations`; pre-002 settings.json records lacking the field
    /// default to `4` (the value previously hard-coded at
    /// `src/src/engine/timer.rs:396` and `:831`).
    #[serde(default = "default_sessions_per_long_break")]
    pub sessions_per_long_break: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            focus_duration: 25,
            break_duration: 5,
            long_break_duration: 20,
            total_sessions: 10,
            weekly_goal_minutes: default_weekly_goal(),
            max_session_time: default_max_session_time(),
            sessions_per_long_break: default_sessions_per_long_break(),
        }
    }
}

/// Default weekly focus goal — 125 minutes per week.
#[must_use]
pub const fn default_weekly_goal() -> u32 {
    125
}

/// Default max single-session time — 120 minutes before auto-pause.
#[must_use]
pub const fn default_max_session_time() -> u32 {
    120
}

/// Default sessions-per-long-break cadence — every 4th focus
/// completion enters long break (matches the pre-002 hard-coded
/// literal in `src/src/engine/timer.rs:396` and `:831`).
#[must_use]
pub const fn default_sessions_per_long_break() -> u32 {
    4
}

/// Notification preferences.
///
/// `auto_start_focus` and `allow_continuous_sessions` carry
/// `#[serde(default)]` because they were added after the `0.4.0`
/// settings shape and may be missing from older settings JSONs.
///
/// `clippy::struct_excessive_bools` is silenced because every bool
/// maps to an independent UI toggle; collapsing them into a state
/// machine would not match the on-disk JSON or the settings UI.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NotificationSettings {
    pub desktop_notifications: bool,
    pub sound_notifications: bool,
    pub auto_start_timer: bool,
    #[serde(default)]
    pub auto_start_focus: bool,
    #[serde(default)]
    pub allow_continuous_sessions: bool,
    pub smart_pause: bool,
    /// Seconds.
    pub smart_pause_timeout: u32,
    /// When true, fire a soft tick once per second during focus
    /// sessions, in sync with the 1 Hz countdown. Default `false`
    /// (opt-in per Principle II). UI-side side effect only — engine
    /// is unaware. Locked to the second; not user-configurable.
    #[serde(default)]
    pub metronome: bool,
    /// Feature 004: when true AND `ambient_sound_type != None`
    /// AND the timer is in the focus running state, the selected
    /// ambient track loops at the configured volume. Default `false`
    /// (opt-in per Principle II). UI-side side effect only — engine
    /// is unaware.
    #[serde(default)]
    pub ambient_sound_enabled: bool,
    /// Feature 004: currently-selected ambient track. `None` is a
    /// first-class "no track selected" sentinel (FR-002 / A5 /
    /// Principle III). Toggling `ambient_sound_enabled` off OR
    /// picking `None` from the dropdown both halt playback while
    /// preserving the other field's value (FR-005).
    #[serde(default)]
    pub ambient_sound_type: AmbientSoundType,
    /// Feature 004: output amplitude, 0..=100 inclusive. Clamped at
    /// the Settings UI input boundary (`<input type="range" min="0"
    /// max="100">`); the audio call site reads the stored value and
    /// passes it through to `HtmlAudioElement::set_volume` without
    /// re-clamping (Principle III — validate at boundaries only).
    /// Default `50` per FR-003 / A9.
    #[serde(default = "default_ambient_sound_volume")]
    pub ambient_sound_volume: u32,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            sound_notifications: true,
            auto_start_timer: true,
            auto_start_focus: false,
            allow_continuous_sessions: false,
            smart_pause: false,
            smart_pause_timeout: 30,
            metronome: false,
            // Feature 004 additions — opt-in defaults.
            ambient_sound_enabled: false,
            ambient_sound_type: AmbientSoundType::None,
            ambient_sound_volume: 50,
        }
    }
}

/// Advanced / debug toggles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AdvancedSettings {
    #[serde(default)]
    pub debug_mode: bool,
}

/// Full application settings record.
///
/// **Wire shape (post-Phase-3a)**: the legacy `hide_status_bar: bool`
/// field is replaced by `status_bar_display: StatusBarDisplay` per
/// the F1/M3 lockstep migration (Phase 3a T150 / T152). Legacy 0.4.x
/// settings JSONs that still carry `hide_status_bar` are read by the
/// `#[serde(from = "SettingsOnDisk")]` shim below: it accepts either
/// shape on the wire, projects through the legacy fallback, and the
/// derived `Serialize` impl then emits only the new shape on next
/// save (legacy field is gone — no field exists for it).
///
/// `clippy::struct_excessive_bools` allowance: every bool is an
/// independent settings toggle exposed in the UI; restructuring
/// would not match the JSON shape on disk or the settings page.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(from = "SettingsOnDisk")]
pub struct Settings {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    pub autostart: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    pub status_bar_display: StatusBarDisplay,
    /// Update versions the user has dismissed from the update-banner.
    /// `#[serde(default)]` so 0.4.x settings JSONs predating this field
    /// still deserialise into the cold-start shape.
    #[serde(default)]
    pub skipped_versions: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcuts: ShortcutSettings::default(),
            timer: TimerSettings::default(),
            notifications: NotificationSettings::default(),
            advanced: AdvancedSettings::default(),
            appearance: AppearanceSettings::default(),
            autostart: false,
            hide_icon_on_close: false,
            status_bar_display: StatusBarDisplay::Default,
            skipped_versions: Vec::new(),
        }
    }
}

/// On-disk shape of the settings JSON, accepting either the new
/// `status_bar_display: StatusBarDisplay` field or the legacy
/// `hide_status_bar: bool` field.
///
/// Used as the `#[serde(from = "SettingsOnDisk")]` source for
/// `Settings`; the `From<SettingsOnDisk> for Settings` impl below
/// ports the legacy fallback from
/// `src/managers/settings-manager.js:109-119`:
///
/// 1. If `status_bar_display` is present, use it.
/// 2. Else if `hide_status_bar: true`, use `IconOnly`.
/// 3. Else if `hide_status_bar: false`, use `Default`.
/// 4. Else, use `StatusBarDisplay::default()`.
///
/// Tie-breaker: when both fields are present, the new field wins.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SettingsOnDisk {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    pub autostart: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    #[serde(default)]
    pub status_bar_display: Option<StatusBarDisplay>,
    /// Legacy read-only fallback. Never re-emitted on save.
    #[serde(default)]
    pub hide_status_bar: Option<bool>,
    #[serde(default)]
    pub skipped_versions: Vec<String>,
}

impl From<SettingsOnDisk> for Settings {
    fn from(raw: SettingsOnDisk) -> Self {
        let status_bar_display = raw.status_bar_display.unwrap_or(match raw.hide_status_bar {
            Some(true) => StatusBarDisplay::IconOnly,
            Some(false) | None => StatusBarDisplay::Default,
        });
        Self {
            shortcuts: raw.shortcuts,
            timer: raw.timer,
            notifications: raw.notifications,
            advanced: raw.advanced,
            appearance: raw.appearance,
            autostart: raw.autostart,
            hide_icon_on_close: raw.hide_icon_on_close,
            status_bar_display,
            skipped_versions: raw.skipped_versions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AmbientSoundType, AppearanceSettings, Locale, NotificationSettings, ShortcutSettings,
        StatusBarDisplay, TimerSettings,
    };

    /// Feature 007 T010 (RED → T013 GREEN): `ShortcutSettings.abort = Some(_)`
    /// round-trips byte-stable through serde. Critical wire-format invariant:
    /// the new field serialises alongside the existing three nullable fields
    /// without altering their wire shape.
    #[test]
    fn shortcut_settings_with_abort_roundtrips() {
        let s = ShortcutSettings {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: Some("CommandOrControl+Alt+R".to_string()),
            skip: Some("CommandOrControl+Alt+S".to_string()),
            abort: Some("CommandOrControl+Alt+W".to_string()),
        };
        let json = serde_json::to_string(&s).expect("serialise");
        let back: ShortcutSettings = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.start_stop.as_deref(), Some("CommandOrControl+Alt+Space"));
        assert_eq!(back.reset.as_deref(), Some("CommandOrControl+Alt+R"));
        assert_eq!(back.skip.as_deref(), Some("CommandOrControl+Alt+S"));
        assert_eq!(back.abort.as_deref(), Some("CommandOrControl+Alt+W"));
    }

    /// Feature 007 T011 (RED → T013 GREEN): `ShortcutSettings.abort = None`
    /// serialises to JSON `null` and deserialises back to `None`. Mirrors the
    /// `Option<String>` precedent of the three sibling fields.
    #[test]
    fn shortcut_settings_with_unbound_abort_roundtrips() {
        let s = ShortcutSettings {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: None,
            skip: None,
            abort: None,
        };
        let json = serde_json::to_string(&s).expect("serialise");
        assert!(
            json.contains(r#""abort":null"#),
            "abort: None must serialise to `null`: got {json}"
        );
        let back: ShortcutSettings = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.abort, None);
    }

    /// Feature 007 T012 (RED → T013 GREEN): a pre-feature settings JSON
    /// (no `abort` key) deserialises with `abort: None` via serde's default
    /// behaviour on a missing `Option<T>` field. Backwards compatibility for
    /// existing users' settings.json files.
    #[test]
    fn shortcut_settings_missing_abort_field_defaults_to_none() {
        let legacy = r#"{
            "start_stop": "CommandOrControl+Alt+Space",
            "reset": "CommandOrControl+Alt+R",
            "skip": "CommandOrControl+Alt+S"
        }"#;
        let s: ShortcutSettings =
            serde_json::from_str(legacy).expect("legacy shortcuts JSON must deserialise");
        assert_eq!(s.abort, None);
        assert_eq!(s.start_stop.as_deref(), Some("CommandOrControl+Alt+Space"));
    }

    /// Feature 005 T004 (RED → T007 GREEN): pre-feature-005
    /// `AppearanceSettings` JSON (feature 002/003/004 baseline shape)
    /// lacking the `locale` key deserialises to `None`. Mirrors the
    /// `ambient_sound_legacy_fields_default` precedent at `:407-421`.
    ///
    /// Critical Fix A invariant: `None` (no explicit choice) MUST be
    /// distinguishable from `Some(Locale::En)` (explicit English). A
    /// German-OS user who explicitly picks English persists `Some(En)`;
    /// the resolver sees `Some(_)` and skips OS detection on next boot.
    #[test]
    fn locale_legacy_field_defaults_to_none() {
        let legacy = r#"{"theme":"auto","timer_theme":"espresso"}"#;
        let s: AppearanceSettings =
            serde_json::from_str(legacy).expect("deserialise legacy appearance");
        assert_eq!(s.locale, None, "legacy record has no explicit locale");
        assert_ne!(
            s.locale,
            Some(Locale::En),
            "Fix A: None must be distinct from Some(En)"
        );
        assert_eq!(s.theme, "auto", "feature-002 theme survives");
        assert_eq!(
            s.timer_theme, "espresso",
            "feature-002 timer_theme survives"
        );
    }

    /// Feature 005 T005 (RED → T007 GREEN): each non-default
    /// `Locale` variant round-trips byte-stable through serde when
    /// stored on `AppearanceSettings.locale`. Critically asserts the
    /// Fix A invariant: `Some(Locale::En)` round-trips as `Some(En)`,
    /// NOT `None` — explicit English MUST persist as a distinct value
    /// from "no explicit choice" so the resolver bypasses OS detection
    /// on next cold start.
    ///
    /// Also asserts feature-002/003/004 `theme` and `timer_theme`
    /// fields survive each round-trip alongside the new `locale`
    /// field (Spec Story 2 AC 5 / SC-003).
    #[test]
    fn locale_some_round_trip() {
        let variants = [Locale::En, Locale::De, Locale::It, Locale::Tr];
        for variant in variants {
            let s = AppearanceSettings {
                theme: "auto".to_string(),
                timer_theme: "espresso".to_string(),
                locale: Some(variant),
            };
            let json = serde_json::to_string(&s).expect("serialise");
            let back: AppearanceSettings = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(
                back.locale,
                Some(variant),
                "Some({variant:?}) round-trips as Some, not None"
            );
            assert_eq!(back.theme, "auto", "theme survives round-trip");
            assert_eq!(
                back.timer_theme, "espresso",
                "timer_theme survives round-trip"
            );
        }
    }

    /// Feature 005 T006 (RED → T007 GREEN): every `Locale` variant
    /// MUST serialise to its lowercase wire string (`"en"` / `"de"` /
    /// `"it"` / `"tr"`) and round-trip in both directions. Wire shape
    /// matches the existing `theme` field's lowercase convention at
    /// `:121-123` rather than the `AmbientSoundType` kebab-case
    /// precedent (two-letter ISO-639-1 codes have no internal word
    /// boundary that kebab-case would clarify).
    ///
    /// Out-of-set wire values (`"fr"`, etc.) MUST fail enum
    /// deserialisation — the `#[serde(default)]` attribute on the
    /// `locale` field then substitutes `None` at the parent struct
    /// level (asserted via the field-level path in T004 /
    /// `locale_legacy_field_defaults_to_none`).
    #[test]
    fn locale_serialises_lowercase() {
        let cases = [
            (Locale::En, r#""en""#),
            (Locale::De, r#""de""#),
            (Locale::It, r#""it""#),
            (Locale::Tr, r#""tr""#),
        ];
        for (variant, wire) in cases {
            let encoded = serde_json::to_string(&variant).expect("serialise");
            assert_eq!(encoded, wire, "serialise {variant:?} → {wire}");
            let decoded: Locale = serde_json::from_str(wire).expect("deserialise");
            assert_eq!(decoded, variant, "deserialise {wire} → {variant:?}");
        }
        // Out-of-set wire value must fail enum deserialisation outright.
        assert!(
            serde_json::from_str::<Locale>(r#""fr""#).is_err(),
            "unsupported locale code fails serde"
        );
    }

    /// Feature 005 Story 2 AC 4: when `settings.json` carries an
    /// out-of-set `locale` value (e.g. hand-edited to `"fr"`), the
    /// `AppearanceSettings` struct MUST deserialise — silently
    /// degrading the unknown value to `None` so the resolver falls
    /// back to OS detection. The companion `Locale`-only test above
    /// guards the inner enum's strictness; this test guards the
    /// field-level leniency. Two separate contracts.
    #[test]
    fn locale_invalid_value_falls_back_to_none() {
        let invalid = r#"{"theme":"auto","timer_theme":"espresso","locale":"fr"}"#;
        let s: AppearanceSettings =
            serde_json::from_str(invalid).expect("invalid locale must not error");
        assert_eq!(s.locale, None);
        assert_eq!(s.theme, "auto");
        assert_eq!(s.timer_theme, "espresso");
    }

    /// Feature 004 T004 (RED → T007 GREEN): pre-feature-004
    /// `NotificationSettings` JSON (no ambient fields) deserialises
    /// to the documented defaults. Mirrors `metronome_default_off`
    /// at `:362-375` verbatim — same fixture shape minus the new
    /// `ambient_sound_*` keys.
    #[test]
    fn ambient_sound_legacy_fields_default() {
        let legacy = r#"{
            "desktop_notifications": true,
            "sound_notifications": true,
            "auto_start_timer": true,
            "smart_pause": false,
            "smart_pause_timeout": 30,
            "metronome": false
        }"#;
        let n: NotificationSettings = serde_json::from_str(legacy).expect("deserialise legacy");
        assert!(!n.ambient_sound_enabled);
        assert_eq!(n.ambient_sound_type, AmbientSoundType::None);
        assert_eq!(n.ambient_sound_volume, 50);
    }

    /// Feature 004 T005 (RED → T007 GREEN): non-default ambient
    /// values (`enabled=true`, `type=WhiteNoise`, `volume=70`)
    /// round-trip byte-stable through serde, AND the feature-002
    /// `metronome: true` field survives the round-trip alongside
    /// the new fields (Acceptance Scenario 2.6).
    #[test]
    fn ambient_sound_round_trip() {
        let n = NotificationSettings {
            metronome: true,
            ambient_sound_enabled: true,
            ambient_sound_type: AmbientSoundType::WhiteNoise,
            ambient_sound_volume: 70,
            ..NotificationSettings::default()
        };
        let json = serde_json::to_string(&n).expect("serialise");
        let back: NotificationSettings = serde_json::from_str(&json).expect("deserialise");
        assert!(
            back.metronome,
            "metronome (feature 002) survives round-trip"
        );
        assert!(back.ambient_sound_enabled);
        assert_eq!(back.ambient_sound_type, AmbientSoundType::WhiteNoise);
        assert_eq!(back.ambient_sound_volume, 70);
    }

    /// Feature 004 T006 (RED → T007 GREEN): every `AmbientSoundType`
    /// variant MUST serialise to its kebab-case wire string and
    /// round-trip in both directions. The `WhiteNoise → "white-noise"`
    /// pair is the critical multi-word case — a misconfigured
    /// `#[serde(rename_all = ...)]` would silently encode it as
    /// `"whitenoise"` or `"white_noise"`.
    #[test]
    fn ambient_sound_type_serialises_kebab_case() {
        let pairs: &[(AmbientSoundType, &str)] = &[
            (AmbientSoundType::None, r#""none""#),
            (AmbientSoundType::Rain, r#""rain""#),
            (AmbientSoundType::Fire, r#""fire""#),
            (AmbientSoundType::Library, r#""library""#),
            (AmbientSoundType::Fan, r#""fan""#),
            (AmbientSoundType::Storm, r#""storm""#),
            (AmbientSoundType::WhiteNoise, r#""white-noise""#),
            (AmbientSoundType::Wind, r#""wind""#),
            (AmbientSoundType::PinkNoise, r#""pink-noise""#),
            (AmbientSoundType::BrownNoise, r#""brown-noise""#),
            (AmbientSoundType::Binaural, r#""binaural""#),
        ];
        for (variant, wire) in pairs {
            let encoded = serde_json::to_string(variant).expect("serialise");
            assert_eq!(&encoded, wire, "serialise {variant:?}");
            let decoded: AmbientSoundType = serde_json::from_str(wire).expect("deserialise");
            assert_eq!(&decoded, variant, "deserialise {wire}");
        }
    }

    /// T005 (RED → T006 GREEN): pre-002 settings.json without the
    /// `sessions_per_long_break` field deserialises to the default `4`
    /// (preserves bit-for-bit engine behaviour on the legacy load
    /// path per data-model.md §Evolution 3 / SC-006).
    #[test]
    fn sessions_per_long_break_default_4() {
        let legacy = r#"{
            "focus_duration": 25,
            "break_duration": 5,
            "long_break_duration": 20,
            "total_sessions": 10
        }"#;
        let s: TimerSettings = serde_json::from_str(legacy).expect("deserialise legacy");
        assert_eq!(s.sessions_per_long_break, 4);
        // Existing default left untouched by the new field's addition.
        assert_eq!(s.weekly_goal_minutes, 125);
    }

    /// T005 (RED → T006 GREEN): a custom `sessions_per_long_break`
    /// value (e.g. 3) round-trips byte-stable through serde.
    #[test]
    fn sessions_per_long_break_custom_round_trips() {
        let s = TimerSettings {
            sessions_per_long_break: 3,
            ..TimerSettings::default()
        };
        let json = serde_json::to_string(&s).expect("serialise");
        let back: TimerSettings = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.sessions_per_long_break, 3);
    }

    /// Pre-002 settings.json `NotificationSettings` lacking the
    /// `metronome` field deserialises to `false`. The legacy fixture
    /// also omits `auto_start_focus` / `allow_continuous_sessions` so
    /// existing `#[serde(default)]` paths stay covered.
    #[test]
    fn metronome_default_off() {
        let legacy = r#"{
            "desktop_notifications": true,
            "sound_notifications": true,
            "auto_start_timer": true,
            "smart_pause": false,
            "smart_pause_timeout": 30
        }"#;
        let n: NotificationSettings = serde_json::from_str(legacy).expect("deserialise legacy");
        assert!(!n.metronome);
        assert!(!n.auto_start_focus);
        assert!(!n.allow_continuous_sessions);
    }

    /// Pre-002 settings.json carrying the now-removed `metronome_bpm`
    /// field still loads — serde drops unknown fields silently
    /// (struct is not `#[serde(deny_unknown_fields)]`). This guards
    /// the upgrade path for any user who already opted in under the
    /// shipped 002 build.
    #[test]
    fn metronome_bpm_legacy_field_ignored() {
        let legacy = r#"{
            "desktop_notifications": true,
            "sound_notifications": true,
            "auto_start_timer": true,
            "smart_pause": false,
            "smart_pause_timeout": 30,
            "metronome": true,
            "metronome_bpm": 90
        }"#;
        let n: NotificationSettings = serde_json::from_str(legacy).expect("deserialise legacy");
        assert!(n.metronome);
    }

    /// Custom metronome enable round-trips byte-stable through serde.
    #[test]
    fn metronome_custom_round_trips() {
        let n = NotificationSettings {
            metronome: true,
            ..NotificationSettings::default()
        };
        let json = serde_json::to_string(&n).expect("serialise");
        let back: NotificationSettings = serde_json::from_str(&json).expect("deserialise");
        assert!(back.metronome);
    }

    #[test]
    fn status_bar_display_default_is_default() {
        assert_eq!(StatusBarDisplay::default(), StatusBarDisplay::Default);
    }

    #[test]
    fn status_bar_display_serialises_kebab() {
        assert_eq!(
            serde_json::to_string(&StatusBarDisplay::Default).unwrap(),
            r#""default""#
        );
        assert_eq!(
            serde_json::to_string(&StatusBarDisplay::IconOnly).unwrap(),
            r#""icon-only""#
        );
    }

    #[test]
    fn status_bar_display_round_trips_kebab() {
        for (json, variant) in [
            (r#""default""#, StatusBarDisplay::Default),
            (r#""icon-only""#, StatusBarDisplay::IconOnly),
        ] {
            let decoded: StatusBarDisplay = serde_json::from_str(json).unwrap();
            assert_eq!(decoded, variant);
        }
    }
}
