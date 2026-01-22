# Upstream Sync & Auto-Release Workflow

This document explains how Antigravity Manager Supreme automatically syncs with the upstream repository, translates code, preserves customizations, and builds releases.

## 🎯 Overview Flow

```mermaid
graph TB
    subgraph "Every 6 Hours (GitHub Actions)"
        A[Scheduled Trigger] --> B{New Upstream Release?}
        B -->|Yes| C[Fetch Upstream]
        B -->|No| Z[Skip - Already Synced]
    end

    subgraph "Sync Process"
        C --> D[Backup Protected Files]
        D --> E[Merge Upstream Code]
        E --> F[Restore Customizations]
        F --> G[Translate Chinese → English]
        G --> H[Update README Links]
    end

    subgraph "Release Process"
        H --> I[Create New Tag v1.X.Y]
        I --> J[Push to GitHub]
        J --> K[Trigger Release Workflow]
    end

    subgraph "Multi-Platform Build"
        K --> L[Build Windows MSI/EXE]
        K --> M[Build macOS DMG Intel/ARM]
        K --> N[Build Linux DEB/RPM/AppImage]
        L --> O[Publish Release]
        M --> O
        N --> O
    end

    style A fill:#4CAF50,color:white
    style O fill:#2196F3,color:white
    style Z fill:#9E9E9E,color:white
```

## ⏰ Schedule

| Trigger | Time (UTC) | Description |
|---------|------------|-------------|
| Scheduled | 0:00, 6:00, 12:00, 18:00 | Automatic check every 6 hours |
| Manual | Any time | "Run workflow" button in Actions |

## 📋 Detailed Steps

### 1. Check for New Upstream Release
```mermaid
sequenceDiagram
    participant GH as GitHub Actions
    participant UP as Upstream Repo
    participant LS as .last-synced-upstream

    GH->>UP: Query latest release
    UP-->>GH: v3.3.37
    GH->>LS: Read last synced version
    LS-->>GH: v3.3.35
    GH->>GH: Compare versions
    Note over GH: v3.3.37 > v3.3.35 → New release!
```

### 2. Protected Files (Customizations Preserved)

These files are backed up before merge and restored after:

| Category | Files |
|----------|-------|
| **Workflows** | `.github/workflows/release.yml`, `sync-upstream.yml` |
| **Documentation** | `README.md`, `CUSTOMIZATIONS.md`, `SYNC_WORKFLOW.md` |
| **UI Pages** | `Dashboard.tsx`, `Settings.tsx`, `Accounts.tsx`, `ApiProxy.tsx`, `Monitor.tsx` |
| **Dashboard Components** | `src/components/dashboard/` (all files) |
| **Backend Commands** | `environment.rs`, `mod.rs`, `autostart.rs`, `proxy.rs` |
| **Tauri Modules** | `updater.rs`, `tray.rs` |
| **Config** | `tauri.conf.json`, `package.json`, `tailwind.config.js` |
| **Scripts** | `scripts/translate-chinese.py` |
| **Tracking** | `.last-synced-upstream` |

### 3. Translation Process

```mermaid
flowchart LR
    A[Rust Source Files] --> B{translate-chinese.py}
    B -->|"400+ patterns"| C[Offline Translation]
    C --> D{Remaining Chinese?}
    D -->|Yes| E[Google Translate API]
    D -->|No| F[Done]
    E --> F
    F --> G[English Codebase]
```

### 4. Version Mapping

| Upstream Version | Supreme Version | Notes |
|------------------|-----------------|-------|
| v3.3.35 | v1.1.4 | Auto-incremented |
| v3.3.36 | v1.1.5 | Next sync |
| v3.3.37 | v1.1.6 | After that |

## 🔧 Supreme Customizations

### UI/UX Enhancements

```mermaid
graph LR
    subgraph "Original UI"
        A1[Chinese Comments]
        A2[Basic Theme]
        A3[Manual Switching]
    end

    subgraph "Supreme UI"
        B1[English Codebase]
        B2[Dark Theme with shadcn/ui]
        B3[Auto-Switch on Quota Depletion]
        B4[YOLO Mode for CLI]
        B5[Dynamic Badges]
    end

    A1 -.->|Translated| B1
    A2 -.->|Upgraded| B2
    A3 -.->|Enhanced| B3
```

### Feature Additions

| Feature | Description | Location |
|---------|-------------|----------|
| **Auto-Switch** | Switches accounts when quota depletes to 0% | Dashboard |
| **YOLO Mode** | One-command Claude CLI with `--dangerously-skip-permissions` | PowerShell/CMD |
| **Dark Theme** | Professional shadcn/ui dark mode with Antigravity-style colors | App-wide |
| **Dynamic Badges** | Version badges auto-update from GitHub API | README |
| **All-Platform Releases** | Windows, macOS (Intel + ARM), Linux builds | GitHub Releases |

## ✅ Verifying the Workflow

### Check Latest Sync
```bash
cat .last-synced-upstream
# Output: v3.3.35
```

### Check GitHub Actions Status
1. Go to [Actions tab](https://github.com/ai-dev-2024/Antigravity-Manager-Supreme/actions)
2. Look for "Sync Upstream & Translate" workflow
3. ✅ Green = Success | ❌ Red = Needs attention

### Manual Trigger with Force Sync
1. Actions → "Sync Upstream & Translate"
2. "Run workflow"
3. ✅ Check "Force sync" to re-sync even if up-to-date

## 🛠️ Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| "Already synced" | No new upstream release | Expected behavior, wait for next release |
| Push fails | History diverged | Workflow has auto-retry with `--force-with-lease` |
| Translation incomplete | API rate limit | Falls back to 100+ inline patterns |
| Build fails | Tauri/Rust issue | Check release.yml logs |

## 📂 Key Files

| File | Purpose |
|------|---------|
| `.github/workflows/sync-upstream.yml` | Main sync + translate workflow |
| `.github/workflows/release.yml` | Multi-platform build workflow |
| `scripts/translate-chinese.py` | Translation script (400+ mappings) |
| `.last-synced-upstream` | Tracks last synced upstream version |
| `CUSTOMIZATIONS.md` | Documents our custom features |
