import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'PikoClaw',
  description: 'Ultra-lightweight AI agent for developers — written in Rust',
  base: '/docs/',

  // INDEX.md → index.md (macOS case-insensitive fs uses uppercase)
  rewrites: {
    'spec/INDEX.md':        'spec/index.md',
    'design-spec/INDEX.md': 'design-spec/index.md',
  },

  head: [
    ['link', { rel: 'icon', type: 'image/png', href: '/favicon.png' }],
    ['meta', { name: 'theme-color', content: '#ea580c' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'PikoClaw Docs' }],
    ['meta', { property: 'og:image', content: 'https://pikoclaw.com/docs/og.png' }],
  ],

  themeConfig: {
    logo: '/logo.png',
    siteTitle: 'PikoClaw',

    nav: [
      { text: 'Home', link: 'https://pikoclaw.com' },
      { text: 'Feature Specs', link: '/spec/' },
      { text: 'Design Specs', link: '/design-spec/' },
      {
        text: 'GitHub',
        link: 'https://github.com/PikoClaw/PikoClaw',
      },
    ],

    sidebar: {
      '/spec/': [
        {
          text: 'Feature Specs',
          link: '/spec/',
          items: [
            {
              text: '✅ Implemented',
              items: [
                { text: 'API Client & Streaming',        link: '/spec/01_api_client' },
                { text: 'Core Agent Loop',               link: '/spec/02_core_agent_loop' },
                { text: 'Built-in Tools',                link: '/spec/03_tools_builtin' },
                { text: 'Config & CLAUDE.md',            link: '/spec/10_config_claudemd' },
                { text: 'Prompt Caching & Tokens',       link: '/spec/11_prompt_caching' },
                { text: 'Extended Thinking',             link: '/spec/13_extended_thinking' },
                { text: 'Plan Mode',                     link: '/spec/17_plan_mode' },
                { text: 'MCP Resources',                 link: '/spec/28_mcp_resources' },
                { text: 'Cost Tracking',                 link: '/spec/31_cost_tracking' },
              ],
            },
            {
              text: '🔶 In Progress',
              items: [
                { text: 'Advanced Tools',                link: '/spec/04_tools_advanced' },
                { text: 'Permission System',             link: '/spec/05_permissions' },
                { text: 'Session Persistence',           link: '/spec/06_session_persistence' },
                { text: 'Terminal UI (TUI)',             link: '/spec/07_tui' },
                { text: 'Slash Commands & Skills',       link: '/spec/08_slash_commands' },
                { text: 'MCP Integration',               link: '/spec/09_mcp' },
                { text: 'Image & Screenshot Input',      link: '/spec/15_image_input' },
                { text: 'Auto-Compact',                  link: '/spec/20_auto_compact' },
                { text: 'Multi-Agent / Swarm',           link: '/spec/21_multi_agent' },
                { text: 'System Prompt Architecture',    link: '/spec/30_system_prompt_architecture' },
              ],
            },
            {
              text: '❌ Todo',
              items: [
                { text: 'Hooks System',                  link: '/spec/12_hooks_system' },
                { text: 'Vim Mode & Keybindings',        link: '/spec/14_vim_keybindings' },
                { text: 'Memory / Memdir',               link: '/spec/16_memory_memdir' },
                { text: 'Git Worktrees',                 link: '/spec/18_worktrees' },
                { text: 'Background Tasks',              link: '/spec/19_task_system' },
                { text: 'IDE Integration',               link: '/spec/22_ide_integration' },
                { text: 'Bridge & Remote',               link: '/spec/23_bridge_remote' },
                { text: 'Voice Input',                   link: '/spec/24_voice_input' },
                { text: 'Plugin System',                 link: '/spec/25_plugins' },
                { text: 'Output Styles',                 link: '/spec/26_output_styles' },
                { text: 'Cron Scheduler',                link: '/spec/27_cron_scheduler' },
                { text: 'Session Commands',              link: '/spec/29_session_commands' },
                { text: 'Layered Settings',              link: '/spec/32_settings_layers' },
                { text: 'Buddy Companion',               link: '/spec/33_buddy_companion' },
              ],
            },
          ],
        },
      ],

      '/design-spec/': [
        {
          text: 'Design Specs',
          link: '/design-spec/',
          items: [
            { text: 'Color & Theme System',    link: '/design-spec/01_color_theme_system' },
            { text: 'Layout & Spacing',        link: '/design-spec/02_layout_and_spacing' },
            { text: 'Input Bar',               link: '/design-spec/03_input_bar' },
            { text: 'Message Rendering',       link: '/design-spec/04_message_rendering' },
            { text: 'File & Image Upload',     link: '/design-spec/05_file_image_upload' },
            { text: 'Status Bar',              link: '/design-spec/06_status_bar' },
            { text: 'Permission Dialogs',      link: '/design-spec/07_permission_dialogs' },
            { text: 'Notifications & Alerts',  link: '/design-spec/08_notifications_alerts' },
            { text: 'Progress & Loading',      link: '/design-spec/09_progress_loading' },
            { text: 'Welcome & Onboarding',    link: '/design-spec/10_welcome_onboarding' },
            { text: 'Symbols & Glyphs',        link: '/design-spec/11_symbols_glyphs' },
            { text: 'Keyboard & Help',         link: '/design-spec/12_keyboard_help' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/PikoClaw/PikoClaw' },
    ],

    editLink: {
      pattern: 'https://github.com/PikoClaw/PikoClaw/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    lastUpdated: {
      text: 'Last updated',
      formatOptions: { dateStyle: 'short' },
    },

    search: {
      provider: 'local',
    },

    footer: {
      message: 'Released under the Apache 2.0 License.',
      copyright: 'Copyright © 2026 PikoClaw',
    },
  },

  markdown: {
    theme: {
      light: 'github-light',
      dark: 'github-dark',
    },
    lineNumbers: true,
  },
})
