# PikoClaw Design Spec Index

Visual and interaction design specifications for the PikoClaw TUI.
Derived from research into the claude-code TypeScript implementation.

Each file documents the *intended design* for porting to Rust/ratatui.

---

## Files

| File | Topic |
|------|-------|
| [01_color_theme_system.md](01_color_theme_system.md) | Color palettes, theme system, all theme variants |
| [02_layout_and_spacing.md](02_layout_and_spacing.md) | Layout rules, flex model, borders, spacing |
| [03_input_bar.md](03_input_bar.md) | Input bar, typeahead, @mentions, multi-line — **Input History Navigation ✅ v0.5.0** |
| [04_message_rendering.md](04_message_rendering.md) | User/assistant messages, tool use display, diffs — **Syntax highlighting & markdown rendering ✅ v0.8.0** |
| [05_file_image_upload.md](05_file_image_upload.md) | File attachment, image paste, @file syntax — **Text paste chips ✅ v0.8.0** |
| [06_status_bar.md](06_status_bar.md) | Status bar layout, token display, rate limits |
| [07_permission_dialogs.md](07_permission_dialogs.md) | Permission prompts, danger highlighting |
| [08_notifications_alerts.md](08_notifications_alerts.md) | Toasts, banners, error overlays |
| [09_progress_loading.md](09_progress_loading.md) | Spinners, progress bars, streaming text |
| [10_welcome_onboarding.md](10_welcome_onboarding.md) | Welcome screen, theme picker, logo |
| [11_symbols_glyphs.md](11_symbols_glyphs.md) | All Unicode symbols, icons, figures used |
| [12_keyboard_help.md](12_keyboard_help.md) | Help overlay, shortcut display, command list |
