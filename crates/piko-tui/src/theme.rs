use ratatui::style::Color;

/// A complete color palette for one theme.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub label: &'static str,
    /// Brand color (Claude orange or equivalent)
    pub claude: Color,
    /// Permission / info accent
    pub permission: Color,
    /// Input/dialog border
    pub prompt_border: Color,
    /// Main text
    pub text: Color,
    /// Inactive / dimmed text
    pub inactive: Color,
    /// Very subtle text / backgrounds
    pub subtle: Color,
    /// Success green
    pub success: Color,
    /// Error red
    pub error: Color,
    /// Warning yellow
    pub warning: Color,
    /// User message bubble background
    pub user_msg_bg: Color,
    /// Status bar background
    pub status_bg: Color,
    /// Main content area background
    pub bg: Color,
}

// ── Theme definitions ──────────────────────────────────────────────────────

/// Default dark theme — exact Claude Code RGB palette.
pub const DARK: Theme = Theme {
    name: "dark",
    label: "Dark",
    claude: Color::Rgb(215, 119, 87),
    permission: Color::Rgb(177, 185, 249),
    prompt_border: Color::Rgb(136, 136, 136),
    text: Color::Rgb(255, 255, 255),
    inactive: Color::Rgb(153, 153, 153),
    subtle: Color::Rgb(80, 80, 80),
    success: Color::Rgb(78, 186, 101),
    error: Color::Rgb(255, 107, 128),
    warning: Color::Rgb(255, 193, 7),
    user_msg_bg: Color::Rgb(55, 55, 55),
    status_bg: Color::Rgb(20, 20, 20),
    bg: Color::Rgb(13, 13, 13),
};

/// Light theme — soft backgrounds, dark text.
pub const LIGHT: Theme = Theme {
    name: "light",
    label: "Light",
    claude: Color::Rgb(180, 80, 40),
    permission: Color::Rgb(80, 90, 200),
    prompt_border: Color::Rgb(100, 100, 100),
    text: Color::Rgb(20, 20, 20),
    inactive: Color::Rgb(120, 120, 120),
    subtle: Color::Rgb(180, 180, 180),
    success: Color::Rgb(30, 140, 60),
    error: Color::Rgb(200, 40, 60),
    warning: Color::Rgb(160, 110, 0),
    user_msg_bg: Color::Rgb(230, 230, 230),
    status_bg: Color::Rgb(210, 210, 210),
    bg: Color::Rgb(253, 253, 253),
};

/// Dark daltonized — deuteranopia-friendly dark theme (replaces red/green with blue/orange).
pub const DARK_DALTONIZED: Theme = Theme {
    name: "dark-daltonized",
    label: "Dark (Accessible)",
    claude: Color::Rgb(215, 160, 0), // amber instead of orange-red
    permission: Color::Rgb(100, 180, 255), // sky blue
    prompt_border: Color::Rgb(136, 136, 136),
    text: Color::Rgb(255, 255, 255),
    inactive: Color::Rgb(153, 153, 153),
    subtle: Color::Rgb(80, 80, 80),
    success: Color::Rgb(0, 160, 220), // blue replaces green
    error: Color::Rgb(255, 140, 0),   // orange replaces red
    warning: Color::Rgb(220, 220, 0),
    user_msg_bg: Color::Rgb(55, 55, 55),
    status_bg: Color::Rgb(20, 20, 20),
    bg: Color::Rgb(13, 13, 13),
};

/// Light daltonized — deuteranopia-friendly light theme.
pub const LIGHT_DALTONIZED: Theme = Theme {
    name: "light-daltonized",
    label: "Light (Accessible)",
    claude: Color::Rgb(160, 100, 0),
    permission: Color::Rgb(0, 100, 200),
    prompt_border: Color::Rgb(100, 100, 100),
    text: Color::Rgb(20, 20, 20),
    inactive: Color::Rgb(120, 120, 120),
    subtle: Color::Rgb(180, 180, 180),
    success: Color::Rgb(0, 100, 180), // blue replaces green
    error: Color::Rgb(180, 100, 0),   // orange replaces red
    warning: Color::Rgb(140, 120, 0),
    user_msg_bg: Color::Rgb(230, 230, 230),
    status_bg: Color::Rgb(210, 210, 210),
    bg: Color::Rgb(253, 253, 253),
};

/// Dark ANSI — uses only the 16-color ANSI palette (portable, low-color terminals).
pub const DARK_ANSI: Theme = Theme {
    name: "dark-ansi",
    label: "Dark (ANSI)",
    claude: Color::Red,
    permission: Color::Blue,
    prompt_border: Color::DarkGray,
    text: Color::White,
    inactive: Color::Gray,
    subtle: Color::DarkGray,
    success: Color::Green,
    error: Color::LightRed,
    warning: Color::Yellow,
    user_msg_bg: Color::DarkGray,
    status_bg: Color::Black,
    bg: Color::Black,
};

/// Light ANSI — uses only the 16-color ANSI palette on a light terminal.
pub const LIGHT_ANSI: Theme = Theme {
    name: "light-ansi",
    label: "Light (ANSI)",
    claude: Color::Red,
    permission: Color::Blue,
    prompt_border: Color::Gray,
    text: Color::Black,
    inactive: Color::DarkGray,
    subtle: Color::Gray,
    success: Color::Green,
    error: Color::Red,
    warning: Color::Yellow,
    user_msg_bg: Color::Gray,
    status_bg: Color::LightCyan,
    bg: Color::White,
};

// ── Registry ───────────────────────────────────────────────────────────────

pub const ALL_THEMES: &[&Theme] = &[
    &DARK,
    &LIGHT,
    &DARK_DALTONIZED,
    &LIGHT_DALTONIZED,
    &DARK_ANSI,
    &LIGHT_ANSI,
];

/// Resolve a theme by name; falls back to `DARK`.
pub fn by_name(name: &str) -> &'static Theme {
    ALL_THEMES
        .iter()
        .copied()
        .find(|t| t.name == name)
        .unwrap_or(&DARK)
}

/// Return the next theme in the cycle (wraps around).
pub fn next(current: &str) -> &'static Theme {
    let idx = ALL_THEMES
        .iter()
        .position(|t| t.name == current)
        .unwrap_or(0);
    ALL_THEMES[(idx + 1) % ALL_THEMES.len()]
}
