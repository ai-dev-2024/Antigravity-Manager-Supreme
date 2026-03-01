# Supreme Edition — What's Different

This document explains what Antigravity Manager Supreme changes relative to [upstream](https://github.com/lbjlaq/Antigravity-Manager).

## 🎯 Philosophy

Supreme is a **thin branding layer** on top of upstream. We use a "checkout" strategy — every sync replaces the entire codebase with the latest upstream release, then re-applies only:

| Protected Asset | Purpose |
|----------------|---------|
| `.github/workflows/` | Our CI/CD pipelines (release + sync) |
| `README.md` | Supreme homepage with download links & Ko-fi |
| `CUSTOMIZATIONS.md` | This file |
| `SYNC_WORKFLOW.md` | Sync workflow documentation |
| `src-tauri/icons/` | Supreme app icons |
| `scripts/translate-chinese.py` | Optional translation script |
| `.last-synced-upstream` | Tracks last synced upstream version |

## 🔧 Build-Time Changes

These are applied automatically during each sync:

| Change | What | Why |
|--------|------|-----|
| **App Name** | `Antigravity Manager Supreme` | Distinguish from upstream |
| **Identifier** | `com.aidev2024.antigravity-manager-supreme` | Unique app ID |
| **Version** | `v1.1.X` (auto-incremented) | Independent versioning |
| **OAuth** | Env var lookup instead of hardcoded | GitHub Push Protection compliance |

## 🔄 Sync Cadence

- **Frequency:** Every 6 hours (GitHub Actions cron)
- **Strategy:** Full checkout from upstream (never merge/rebase)
- **Trigger:** New upstream release detected via GitHub API
- **Result:** Auto-tagged `v1.1.X` → triggers multi-platform release build

## 💖 Support

<div align="center">
  <a href="https://ko-fi.com/ai_dev_2024">
    <img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="Support on Ko-fi">
  </a>
</div>
