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

/// Parse a single line of text into styled `Span`s, handling inline markdown:
/// - `` `code` `` → `code_style` (blue)
/// - `**bold**` → BOLD modifier
/// - `*italic*` / `_italic_` → ITALIC modifier
pub fn parse_inline_spans(text: &str, text_style: Style, code_style: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0;

    while i < text.len() {
        let remaining = &text[i..];
        let b = remaining.as_bytes()[0];

        if b == b'`' {
            if let Some(close) = remaining[1..].find('`') {
                let code = remaining[1..1 + close].to_owned();
                spans.push(Span::styled(code, code_style));
                i += 1 + close + 1;
                continue;
            }
        }

        if b == b'*' && remaining.starts_with("**") {
            if let Some(close) = remaining[2..].find("**") {
                let bold = remaining[2..2 + close].to_owned();
                if !bold.is_empty() {
                    spans.push(Span::styled(bold, text_style.add_modifier(Modifier::BOLD)));
                }
                i += 2 + close + 2;
                continue;
            } else {
                // No closing ** on this line — treat the rest as bold, or skip if empty.
                // This handles multi-line **bold** where the opener/closer appear alone.
                let rest = &remaining[2..];
                if !rest.is_empty() {
                    spans.push(Span::styled(
                        rest.to_owned(),
                        text_style.add_modifier(Modifier::BOLD),
                    ));
                }
                // consumed all remaining text
                break;
            }
        }

        if b == b'*' && !remaining.starts_with("**") {
            if let Some(close) = remaining[1..].find('*') {
                if close > 0 {
                    let italic = remaining[1..1 + close].to_owned();
                    spans.push(Span::styled(
                        italic,
                        text_style.add_modifier(Modifier::ITALIC),
                    ));
                    i += 1 + close + 1;
                    continue;
                }
            }
        }

        if b == b'_' {
            if let Some(close) = remaining[1..].find('_') {
                if close > 0 {
                    let italic = remaining[1..1 + close].to_owned();
                    spans.push(Span::styled(
                        italic,
                        text_style.add_modifier(Modifier::ITALIC),
                    ));
                    i += 1 + close + 1;
                    continue;
                }
            }
        }

        // Advance to next special character or end
        let start = i;
        loop {
            if i >= text.len() {
                break;
            }
            let ch = text[i..].chars().next().unwrap();
            let cb = ch as u32;
            if cb < 128 && matches!(ch, '`' | '*' | '_') {
                break;
            }
            i += ch.len_utf8();
        }

        if i > start {
            spans.push(Span::styled(text[start..i].to_owned(), text_style));
        } else {
            let ch = text[i..].chars().next().unwrap();
            spans.push(Span::styled(ch.to_string(), text_style));
            i += ch.len_utf8();
        }
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn plain_style() -> Style {
        Style::default().fg(Color::White)
    }

    fn code_style() -> Style {
        Style::default().fg(Color::Blue)
    }

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn plain_text_unchanged() {
        let spans = parse_inline_spans("hello world", plain_style(), code_style());
        assert_eq!(text(&spans), "hello world");
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn inline_code_single() {
        let spans = parse_inline_spans("`foo`", plain_style(), code_style());
        assert_eq!(text(&spans), "foo");
        assert_eq!(spans[0].style.fg, Some(Color::Blue));
    }

    #[test]
    fn inline_code_surrounded() {
        let spans = parse_inline_spans("call `foo` now", plain_style(), code_style());
        assert_eq!(text(&spans), "call foo now");
        assert_eq!(spans[0].content.as_ref(), "call ");
        assert_eq!(spans[1].style.fg, Some(Color::Blue));
        assert_eq!(spans[1].content.as_ref(), "foo");
        assert_eq!(spans[2].content.as_ref(), " now");
    }

    #[test]
    fn bold_text() {
        let spans = parse_inline_spans("**bold**", plain_style(), code_style());
        assert_eq!(text(&spans), "bold");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn italic_star() {
        let spans = parse_inline_spans("*italic*", plain_style(), code_style());
        assert_eq!(text(&spans), "italic");
        assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn italic_underscore() {
        let spans = parse_inline_spans("_italic_", plain_style(), code_style());
        assert_eq!(text(&spans), "italic");
        assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn mixed_inline() {
        let spans = parse_inline_spans("use `foo` and **bar**", plain_style(), code_style());
        assert_eq!(text(&spans), "use foo and bar");
    }

    #[test]
    fn unclosed_backtick_treated_as_plain() {
        let spans = parse_inline_spans("`unclosed", plain_style(), code_style());
        assert_eq!(text(&spans), "`unclosed");
    }

    #[test]
    fn empty_string() {
        let spans = parse_inline_spans("", plain_style(), code_style());
        assert!(spans.is_empty());
    }

    #[test]
    fn parse_segments_plain() {
        let segs = parse_segments("hello world");
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0], Segment::Text("hello world")));
    }

    #[test]
    fn parse_segments_code_block() {
        let input = "text\n```rust\nfn main() {}\n```\nafter";
        let segs = parse_segments(input);
        assert_eq!(segs.len(), 3);
        assert!(matches!(segs[0], Segment::Text(_)));
        assert!(matches!(segs[1], Segment::Code { lang: "rust", .. }));
        assert!(matches!(segs[2], Segment::Text(_)));
    }

    #[test]
    fn parse_segments_no_code() {
        let segs = parse_segments("no code here");
        assert_eq!(segs.len(), 1);
    }
}
