# Antigravity Manager Supreme - Customizations

This document lists all customizations made to the original [Antigravity Manager](https://github.com/lbjlaq/Antigravity-Manager) that are preserved during upstream syncs.

## 🔗 Support

If you find this fork useful, please consider supporting the development:

<div align="center">
  <a href="https://ko-fi.com/ai_dev_2024">
    <img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="Support on Ko-fi">
  </a>
</div>

---

## 📋 Customization Summary

| Category | Feature | Description |
|----------|---------|-------------|
| **Best Account Switches** | Best Claude / Best Gemini | Separate buttons for switching to best account per model |
| **Auto-Switch** | Quota depletion detection | Automatically switches accounts when quota hits 0% |
| **One-Click CLI** | CLI Sync Card | One-click integration with Claude/Codex/Gemini CLIs |
| **Dark Theme** | shadcn/ui colors | Professional dark theme (#1a1f2e) with blue accents |
| **UI Components** | Button/Card styling | Consistent `bg-card`, `border-border`, `text-card-foreground` |
| **Translation** | Chinese → English | 400+ patterns translated to English codebase |
| **Versioning** | v1.X.Y | Custom version scheme (upstream uses v3.3.X) |

---

## 📁 Protected Files

These files are backed up before upstream merge and restored after:

### Documentation
- `README.md` - Custom homepage with download links, Ko-fi support
- `CUSTOMIZATIONS.md` - This file
- `SYNC_WORKFLOW.md` - Workflow documentation

### UI Pages
- `src/pages/Dashboard.tsx` - Stats, best accounts, quick links
- `src/pages/Settings.tsx` - Theme toggle, forms
- `src/pages/Accounts.tsx` - Account management
- `src/pages/ApiProxy.tsx` - Proxy controls, CLI sync
- `src/pages/Monitor.tsx` - Monitoring

### Components
- `src/components/dashboard/` - BestAccounts, CurrentAccount, StatsCard
- `src/components/proxy/CliSyncCard.tsx` - One-click CLI sync
- `src/components/layout/Navbar.tsx` - Navigation styling
- `src/components/common/ThemeManager.tsx` - Theme switching
- `src/components/accounts/AddAccountDialog.tsx` - Add account modal

### Styling
- `src/App.css` - shadcn/ui dark theme CSS variables
- `components.json` - shadcn configuration
- `tailwind.config.js` - Tailwind customizations

### Backend
- `src-tauri/src/commands/` - Environment, proxy, autostart commands
- `src-tauri/src/modules/` - Updater, tray modules
- `src-tauri/tauri.conf.json` - App configuration

### Scripts
- `scripts/translate-chinese.py` - Translation script (400+ patterns)

---

## 🎨 Theme Colors

The dark theme uses these colors:
```css
--background: #1a1f2e    /* Deep blue-gray */
--card: #1e2433          /* Slightly lighter card */
--primary: #3b82f6       /* Vibrant blue */
--border: #2a3142        /* Subtle border */
```

---

## 🔄 Auto-Sync Workflow

See [SYNC_WORKFLOW.md](./SYNC_WORKFLOW.md) for detailed workflow documentation.

---

<div align="center">
  <sub>
    Fork maintained by <a href="https://github.com/ai-dev-2024">ai-dev-2024</a><br>
    <a href="https://ko-fi.com/ai_dev_2024">☕ Support on Ko-fi</a>
  </sub>
</div>
