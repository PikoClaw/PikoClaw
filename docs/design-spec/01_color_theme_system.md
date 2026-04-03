# Design Spec: Color & Theme System

**TS source**: `outputStyles/`, `constants/outputStyles.ts`

---

## Themes Available

| Theme ID | Description |
|----------|-------------|
| `dark` | Default dark mode (RGB colors) |
| `light` | Light mode (RGB colors) |
| `dark-daltonized` | Dark, colorblind-friendly (deuteranopia/protanopia safe) |
| `light-daltonized` | Light, colorblind-friendly |
| `dark-ansi` | Dark using only 16 ANSI colors (max compatibility) |
| `light-ansi` | Light using only 16 ANSI colors |

---

## Light Theme — Full Color Table

```
Category              Key                           Value
─────────────────────────────────────────────────────────────
Brand
                      claude                        rgb(215, 119,  87)   ← Claude orange
                      claudeShimmer                 rgb(245, 149, 117)   ← lighter orange (shimmer)
                      clawd_body                    rgb(215, 119,  87)   ← logo body
                      clawd_background              rgb(  0,   0,   0)   ← logo bg (dark even in light)

Interactive
                      permission                    rgb( 87, 105, 247)   ← medium blue
                      permissionShimmer             rgb(137, 155, 255)   ← lighter blue shimmer
                      suggestion                    rgb( 87, 105, 247)   ← same blue (autocomplete)
                      autoAccept                    rgb( 87, 105, 247)   ← auto-allow indicator

Mode Borders
                      bashBorder                    rgb(255,   0, 135)   ← hot pink (bash mode)
                      planMode                      rgb(  0, 102, 102)   ← muted teal (plan mode)
                      ide                           rgb( 71, 130, 200)   ← muted blue (IDE mode)

Text
                      text                          rgb(  0,   0,   0)   ← black
                      inverseText                   rgb(255, 255, 255)   ← white (inverse mode)
                      inactive                      rgb(102, 102, 102)   ← dark gray (dimmed)
                      subtle                        rgb(175, 175, 175)   ← light gray (very dim)

Status
                      success                       rgb( 44, 122,  57)   ← green
                      error                         rgb(171,  43,  63)   ← dark red/maroon
                      warning                       rgb(150, 108,  30)   ← amber/brown

Diff
                      diffAdded                     rgb(105, 219, 124)   ← light green bg
                      diffAddedDimmed               rgb(199, 225, 203)   ← very light green
                      diffAddedWord                 rgb( 47, 157,  68)   ← medium green (word hl)
                      diffRemoved                   rgb(255, 168, 180)   ← light pink/red bg
                      diffRemovedDimmed             rgb(253, 210, 216)   ← very light pink
                      diffRemovedWord               rgb(209,  69,  75)   ← medium red (word hl)

Message Backgrounds
                      userMessageBackground         rgb(240, 240, 240)   ← light gray
                      userMessageBackgroundHover    rgb(252, 252, 252)   ← near white (hover)
                      messageActionsBackground      rgb(232, 236, 244)   ← cool gray

Special
                      fastMode                      rgb(255, 106,   0)   ← electric orange
                      merged                        rgb(135,   0, 255)   ← electric violet

Rate Limits
                      rate_limit_fill               rgb( 87, 105, 247)   ← blue (used portion)
                      rate_limit_empty              rgb( 39,  47, 111)   ← dark blue (remaining)

Spinner
                      claudeBlue_FOR_SYSTEM_SPINNER       rgb( 87, 105, 247)
                      claudeBlueShimmer_FOR_SYSTEM_SPINNER rgb(117, 135, 255)

Rainbow (thinking blocks)
                      rainbow_red                   rgb(235,  95,  87)
                      rainbow_orange                rgb(245, 139,  87)
                      rainbow_yellow                rgb(250, 195,  95)
                      rainbow_green                 rgb(145, 200, 130)
                      rainbow_blue                  rgb(130, 170, 220)
                      rainbow_indigo                rgb(155, 130, 200)
                      rainbow_violet                rgb(200, 130, 180)

Sub-agent Team Colors (never use for other purposes)
                      red_FOR_SUBAGENTS_ONLY        rgb(220,  38,  38)
                      blue_FOR_SUBAGENTS_ONLY       rgb( 37,  99, 235)
                      green_FOR_SUBAGENTS_ONLY      rgb( 22, 163,  74)
                      yellow_FOR_SUBAGENTS_ONLY     rgb(202, 138,   4)
                      purple_FOR_SUBAGENTS_ONLY     rgb(147,  51, 234)
                      orange_FOR_SUBAGENTS_ONLY     rgb(234,  88,  12)
                      pink_FOR_SUBAGENTS_ONLY       rgb(219,  39, 119)
                      cyan_FOR_SUBAGENTS_ONLY       rgb(  8, 145, 178)
```

---

## Dark Theme Differences

Dark theme inverts text/background relationships and adjusts mid-tones:

```
text                  rgb(255, 255, 255)   ← white
inverseText           rgb(  0,   0,   0)   ← black
inactive              rgb(153, 153, 153)   ← medium gray
subtle                rgb( 80,  80,  80)   ← dark gray
userMessageBackground rgb( 40,  40,  40)   ← dark gray bg
messageActionsBackground rgb(50, 55, 65)   ← dark cool gray
```

Brand, status, diff, and interactive colors remain identical between light and dark — only text/background flip.

---

## ANSI Themes (16-color)

For terminals that don't support true color. Maps semantic names to ANSI color codes:

```
autoAccept      → ansi:magenta
claude          → ansi:redBright
claudeShimmer   → ansi:red
permission      → ansi:blue
permissionShimmer → ansi:blueBright
suggestion      → ansi:blue
success         → ansi:green
error           → ansi:red
warning         → ansi:yellow
inactive        → ansi:blackBright   (bright black = gray)
text            → ansi:white (dark-ansi) / ansi:black (light-ansi)
diffAdded       → ansi:greenBright
diffRemoved     → ansi:redBright
```

---

## Daltonized Themes

Replaces red/green pairs that are indistinguishable to deuteranopes/protanopes:

```
diffAdded     → blue shades instead of green
diffRemoved   → orange/yellow shades instead of red/pink
success       → blue instead of green
error         → orange instead of red
```

Exact values TBD — derive by running the standard red/green colors through a daltonization matrix and picking safe alternatives.

---

## Rust Implementation

```rust
// piko-tui/src/theme.rs

pub struct Theme {
    // Brand
    pub claude: Color,
    pub claude_shimmer: Color,

    // Interactive
    pub permission: Color,
    pub permission_shimmer: Color,
    pub suggestion: Color,

    // Mode borders
    pub bash_border: Color,
    pub plan_mode: Color,

    // Text
    pub text: Color,
    pub inverse_text: Color,
    pub inactive: Color,
    pub subtle: Color,

    // Status
    pub success: Color,
    pub error: Color,
    pub warning: Color,

    // Diff
    pub diff_added: Color,
    pub diff_added_dimmed: Color,
    pub diff_added_word: Color,
    pub diff_removed: Color,
    pub diff_removed_dimmed: Color,
    pub diff_removed_word: Color,

    // Backgrounds
    pub user_message_bg: Color,
    pub message_actions_bg: Color,

    // Special
    pub fast_mode: Color,
    pub rate_limit_fill: Color,
    pub rate_limit_empty: Color,

    // Spinner
    pub spinner: Color,
    pub spinner_shimmer: Color,
}

pub fn dark_theme() -> Theme { ... }
pub fn light_theme() -> Theme { ... }
pub fn dark_daltonized_theme() -> Theme { ... }
pub fn light_daltonized_theme() -> Theme { ... }
pub fn dark_ansi_theme() -> Theme { ... }
pub fn light_ansi_theme() -> Theme { ... }

// Already implemented — verify all fields above are present
```

### ratatui Color Mapping

```rust
// For RGB colors:
Color::Rgb(r, g, b)

// For ANSI colors:
Color::Red, Color::Green, Color::Blue,
Color::LightRed, Color::LightGreen, Color::LightBlue, ...
Color::DarkGray  // = ansi:blackBright
```

### Shimmer Animation

Shimmer is a two-color pulse between `color` and `color_shimmer` at ~500ms interval.

```rust
pub fn shimmer_color(theme: &Theme, tick: u64) -> Color {
    if (tick / 500) % 2 == 0 { theme.spinner } else { theme.spinner_shimmer }
}
```
