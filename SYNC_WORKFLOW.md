# Upstream Sync & Auto-Release Workflow

Technical documentation for the automated sync pipeline.

## Overview

```
Schedule (every 6h) → Check upstream → Checkout code → Apply branding → Clean secrets → Tag → Release
```

## Workflows

### 1. `sync-upstream.yml` — Sync & Brand

| Step | Action |
|------|--------|
| **Check upstream** | Queries GitHub API for latest upstream release tag |
| **Compare versions** | Reads `.last-synced-upstream` to detect new releases |
| **Checkout upstream** | Clones upstream at the release tag (clean slate) |
| **Restore protected files** | Re-applies workflows, README, icons from our branch |
| **Apply branding** | Sets app name, identifier, version in `tauri.conf.json` + `package.json` + `Cargo.toml` |
| **Clean OAuth secrets** | Replaces any hardcoded Google OAuth credentials with `once_cell::Lazy` env var lookups |
| **Force push** | Pushes to `main` (force-with-lease) |
| **Tag** | Creates `v1.1.X` tag → triggers release workflow |

**Triggers:** `schedule` (every 6h) or `workflow_dispatch` (manual)

### 2. `release.yml` — Multi-Platform Build

| Step | Action |
|------|--------|
| **Build matrix** | 4 parallel builds: Windows x64, Linux amd64, macOS aarch64, macOS x64 |
| **Upload artifacts** | Each build uploads its installers as workflow artifacts |
| **Publish release** | Downloads all artifacts, generates `updater.json`, creates GitHub Release |

**Triggers:** Tag push matching `v*` or `workflow_dispatch`

**Build outputs per platform:**

| Platform | Artifacts |
|----------|-----------|
| Windows | `.exe` (NSIS), `.msi` |
| macOS ARM | `_aarch64.dmg` |
| macOS Intel | `_x64.dmg` |
| Linux | `.deb`, `.rpm`, `.AppImage` |

## Protected Files

Files that survive every upstream sync:

```
.github/workflows/release.yml
.github/workflows/sync-upstream.yml
README.md
CUSTOMIZATIONS.md
SYNC_WORKFLOW.md
src-tauri/icons/
scripts/translate-chinese.py
.last-synced-upstream
```

Everything else is replaced with upstream code on each sync.

## Version Mapping

| Our Version | Meaning |
|-------------|---------|
| `v1.1.X` | Supreme release X, synced to latest upstream |

The patch number auto-increments on each successful sync.

## Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| "Already synced" | No new upstream release | Expected — wait for next release |
| Build fails | Upstream introduced breaking change | Check release.yml logs, may need workflow update |
| OAuth push blocked | Hardcoded secrets in upstream | The sync workflow auto-cleans these |
| Sync skipped | `.last-synced-upstream` matches latest | Use "Force sync" in workflow_dispatch |

## Manual Trigger

1. Go to [Actions](https://github.com/ai-dev-2024/Antigravity-Manager-Supreme/actions)
2. Select **"Sync Upstream & Translate"**
3. Click **"Run workflow"**
4. Optionally check **"Force sync"** to re-sync even if up-to-date

## Key Files

| File | Purpose |
|------|---------|
| `.github/workflows/sync-upstream.yml` | Sync + branding + tagging |
| `.github/workflows/release.yml` | Multi-platform build + publish |
| `.last-synced-upstream` | Tracks last synced upstream version |
