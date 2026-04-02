/// Syntax highlighting for fenced code blocks in assistant messages.
///
/// Uses `syntect` (same engine as `bat`) with bundled TextMate grammars and themes.
/// The active PikoClaw theme selects the matching syntect colour theme:
///   dark / dark-daltonized  → "Monokai Extended"   (dark, high-contrast)
///   light / light-daltonized → "InspiredGitHub"     (light, GitHub-like)
///   dark-ansi / light-ansi  → "base16-ocean.dark"  (ANSI-safe fallback)
///
/// Parsing:
///   A message is split into plain-text segments and fenced code blocks (```lang…```).
///   Each code block line is tokenised by syntect and emitted as a `Vec<Span<'static>>`
///   with per-token RGB colours. Plain-text lines are returned unstyled.
use crate::theme::Theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

// ── Lazy-initialised syntect state ────────────────────────────────────────────

struct SyntectState {
    ss: SyntaxSet,
    ts: ThemeSet,
}

static STATE: std::sync::OnceLock<SyntectState> = std::sync::OnceLock::new();

fn state() -> &'static SyntectState {
    STATE.get_or_init(|| SyntectState {
        ss: SyntaxSet::load_defaults_newlines(),
        ts: ThemeSet::load_defaults(),
    })
}

// ── Theme name mapping ────────────────────────────────────────────────────────

/// Choose a syntect theme name that best matches the PikoClaw theme.
fn syntect_theme_name(piko_theme: &str) -> &'static str {
    if piko_theme.contains("ansi") {
        "base16-ocean.dark"
    } else if piko_theme.contains("light") {
        "InspiredGitHub"
    } else {
        "Monokai Extended"
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// A segment of a parsed message.
pub enum Segment<'a> {
    /// Plain text (not inside a code fence).
    Text(&'a str),
    /// A fenced code block: (language_hint, code_body).
    Code { lang: &'a str, body: &'a str },
}

/// Split `content` into alternating plain-text and code-fence segments.
/// Fences are standard markdown: ``` optionally followed by a language name.
pub fn parse_segments(content: &str) -> Vec<Segment<'_>> {
    let mut segments = Vec::new();
    let mut rest = content;

    while let Some(fence_start) = rest.find("```") {
        // Plain text before the fence
        let before = &rest[..fence_start];
        if !before.is_empty() {
            segments.push(Segment::Text(before));
        }
        rest = &rest[fence_start + 3..]; // skip opening ```

        // Collect the language hint (rest of opening line)
        let (lang, after_lang) = match rest.find('\n') {
            Some(nl) => (&rest[..nl], &rest[nl + 1..]),
            None => ("", rest),
        };
        rest = after_lang;

        // Find closing ```
        match rest.find("```") {
            Some(end) => {
                let body = &rest[..end];
                segments.push(Segment::Code {
                    lang: lang.trim(),
                    body,
                });
                rest = &rest[end + 3..];
                // skip any trailing newline after closing fence
                if rest.starts_with('\n') {
                    rest = &rest[1..];
                }
            }
            None => {
                // Unclosed fence — treat remainder as code
                segments.push(Segment::Code {
                    lang: lang.trim(),
                    body: rest,
                });
                rest = "";
            }
        }
    }

    if !rest.is_empty() {
        segments.push(Segment::Text(rest));
    }

    segments
}

/// Highlight one fenced code block and return ratatui `Line`s ready for rendering.
/// Each source line becomes one `Line` containing coloured `Span`s.
/// The `indent` prefix (e.g. `"  "` for 2-char assistant indent) is prepended
/// to every line as a plain span.
pub fn highlight_code(
    lang: &str,
    body: &str,
    piko_theme: &Theme,
    indent: &'static str,
) -> Vec<Line<'static>> {
    let st = state();
    let theme_name = syntect_theme_name(piko_theme.name);

    // Resolve syntax — fall back to plain text if language unknown
    let syntax = if lang.is_empty() {
        st.ss.find_syntax_plain_text()
    } else {
        st.ss
            .find_syntax_by_token(lang)
            .or_else(|| st.ss.find_syntax_by_extension(lang))
            .unwrap_or_else(|| st.ss.find_syntax_plain_text())
    };

    let syntect_theme = st
        .ts
        .themes
        .get(theme_name)
        .or_else(|| st.ts.themes.get("base16-ocean.dark"))
        .expect("syntect built-in themes must exist");

    let mut hl = HighlightLines::new(syntax, syntect_theme);
    let mut lines: Vec<Line<'static>> = Vec::new();

    for line_str in LinesWithEndings::from(body) {
        let tokens = match hl.highlight_line(line_str, &st.ss) {
            Ok(t) => t,
            Err(_) => {
                // On error just emit plain line
                let text = line_str.trim_end_matches('\n').to_owned();
                lines.push(Line::from(vec![
                    Span::raw(indent),
                    Span::styled(text, Style::default().fg(piko_theme.text)),
                ]));
                continue;
            }
        };

        let mut spans: Vec<Span<'static>> = vec![Span::raw(indent)];
        for (style, text) in tokens {
            let fg = syntect_color_to_ratatui(style.foreground);
            let text = text.trim_end_matches('\n').to_owned();
            if text.is_empty() {
                continue;
            }
            let mut ratatui_style = Style::default().fg(fg);
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::BOLD)
            {
                ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
            }
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::ITALIC)
            {
                ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
            }
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::UNDERLINE)
            {
                ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
            }
            spans.push(Span::styled(text, ratatui_style));
        }

        lines.push(Line::from(spans));
    }

    lines
}

/// Convert a syntect `Color` (RGBA) to a ratatui `Color`.
fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> Color {
    // syntect uses (r, g, b, a) — a=0 means "use terminal default"
    if c.a == 0 {
        Color::Reset
    } else {
        Color::Rgb(c.r, c.g, c.b)
    }
}
