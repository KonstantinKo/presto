// Timer-theme metadata — display name, description, supported color
// modes, and preview colors. Mirrors the JS-era `TIMER_THEMES` table
// at `src/utils/timer-themes.js`. The auto-generated `themes::ALL_THEMES`
// catalogue carries only the stem ids; this hand-written table layers
// on the visual metadata that `ThemeSettings` renders per tile.

/// Light vs dark mode compatibility bitmask. Mirrors the JS-era
/// `supports: ("light" | "dark")[]` array.
#[derive(Debug, Clone, Copy)]
pub struct Supports {
    pub light: bool,
    pub dark: bool,
}

/// Per-mode accent colors shown in the tile's color strip and timer
/// preview. Mirrors the JS-era `preview.{focus,break,longBreak}` keys.
#[derive(Debug, Clone, Copy)]
pub struct Preview {
    pub focus: &'static str,
    pub break_: &'static str,
    pub long_break: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub supports: Supports,
    pub preview: Preview,
}

const DEFAULT_PREVIEW: Preview = Preview {
    focus: "#e74c3c",
    break_: "#2ecc71",
    long_break: "#3498db",
};

/// Three-entry table mirroring `TIMER_THEMES`.
///
/// Sourced from `src/utils/timer-themes.js:14-45`. A future code-gen
/// pass can derive this from theme-CSS metadata blocks, but the
/// small closed set + the JS-era manual catalogue make a hand-table
/// the pragmatic choice today.
pub const THEME_METADATA: &[ThemeMeta] = &[
    ThemeMeta {
        id: "espresso",
        name: "Espresso",
        description: "Warm, coffee-inspired colors with rich earth tones",
        supports: Supports { light: true, dark: true },
        preview: DEFAULT_PREVIEW,
    },
    ThemeMeta {
        id: "pommodore64",
        name: "Pommodore64",
        description: "Un tema retr\u{f2} ispirato al Commodore 64 con colori nostalgici e font pixelato",
        supports: Supports { light: true, dark: false },
        preview: Preview {
            focus: "#6c5ce7",
            break_: "#0984e3",
            long_break: "#00b894",
        },
    },
    ThemeMeta {
        id: "pipboy",
        name: "PipBoy",
        description: "A retro-futuristic theme inspired by Fallout's PipBoy interface with green terminal colors and digital effects",
        supports: Supports { light: false, dark: true },
        preview: Preview {
            focus: "#00ff41",
            break_: "#39ff14",
            long_break: "#00cc33",
        },
    },
];

/// Look up metadata by stem id. Returns `None` if the id is not in
/// the table (the caller should fall back to a default; the JS-era
/// `getThemeById` returns the `espresso` entry as the fallback).
#[must_use]
pub fn by_id(id: &str) -> Option<&'static ThemeMeta> {
    THEME_METADATA.iter().find(|m| m.id == id)
}

/// `true` if `id` is renderable under the current OS / explicit
/// color mode (`"light"` or `"dark"`). Mirrors the JS-era
/// `isThemeCompatible` body at `timer-themes.js:106`.
#[must_use]
pub fn is_compatible(id: &str, color_mode: &str) -> bool {
    let Some(meta) = by_id(id) else { return false };
    match color_mode {
        "dark" => meta.supports.dark,
        _ => meta.supports.light,
    }
}
