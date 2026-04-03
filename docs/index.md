---
layout: home

hero:
  name: PikoClaw
  text: Documentation
  tagline: Ultra-lightweight AI agent for developers — written in Rust
  image:
    src: /logo.png
    alt: PikoClaw
  actions:
    - theme: brand
      text: Feature Specs
      link: /spec/
    - theme: alt
      text: Design Specs
      link: /design-spec/
    - theme: alt
      text: GitHub
      link: https://github.com/PikoClaw/PikoClaw

features:
  - icon: ⚙️
    title: Feature Specs
    details: Technical specifications for every feature — what's implemented, what's todo, and detailed implementation plans for porting from TypeScript to Rust.
    link: /spec/
    linkText: Browse feature specs

  - icon: 🎨
    title: Design Specs
    details: Visual and interaction design research derived from the claude-code source. Colors, layout, animations, symbols — everything needed to build the TUI.
    link: /design-spec/
    linkText: Browse design specs

  - icon: 🦀
    title: Built in Rust
    details: 10-crate workspace, async throughout with Tokio, ratatui TUI, full Anthropic API streaming, MCP support, and prompt caching.
    link: https://github.com/PikoClaw/PikoClaw
    linkText: View source

  - icon: 📦
    title: ~6–7 MB binary
    details: Single static binary, no Node.js, no npm, no runtime dependencies. Installs in seconds.
---
