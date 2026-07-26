# DozeForge

> Forge your own power-management rules for Android. Surgical battery and CPU auditor over ADB, with atomic rollback and zero root.

**Status:** Beta 1 (`1.0.0-beta.1`) · **Target:** Android 12+ (API 31..35) · **Stack:** Tauri 2 · Rust 1.79+ · SvelteKit 2 · Svelte 5

---

## Why DozeForge

Modern Android leaks battery not because of "bad apps" you can guess from a blocklist, but because of:

1. **OEM bloatware** that survives every reboot and ignores Doze (Samsung Health, MIUI Cleaner, Xiaomi GetApps...).
2. **Third-party apps proxying through Google Play Services** (FCM, JobScheduler) so the symptom shows up as `com.google.android.gms` while the culprit is hidden.
3. **Phantom-process killer (API 31+)** that nukes legitimate background tools (Termux, Tasker) while OEM background services keep running fine.

DozeForge reads the actual on-device telemetry over ADB, traces the real culprit (not the proxy), applies progressive restrictions using only public `cmd appops` / `am set-standby-bucket` / `pm disable-user` primitives, snapshots the prior state, and lets you roll back atomically -- or export everything as a Termux/Shizuku shell script so the rules survive without ever needing the PC again.

## What's inside (Beta 1)

DozeForge ships as a frameless desktop app with a global command palette (`Ctrl/Cmd + K`), light/dark themes, and full English / Spanish (EN/ES) localization. The workspace is organized into:

| Area | Route | What it does |
|------|-------|--------------|
| **Overview** | `/` | Snapshot of the selected device: health, sleep score, top offenders |
| **Fleet** | `/fleet/` | Bulk actions across many attached devices at once |
| **Doze & Sleep** | `/sleep/` | Wakelock analysis, sleep timeline, culprit ranking |
| **Battery** | `/battery/` | Health, cycles, sysfs, per-app drain, historical charts |
| **Storage** | `/storage/` | Inventory by code size, cache trim, background dexopt |
| **Network & DNS** | `/network/` | Private DNS presets, data saver, per-app firewall |
| **System Tweaks** | `/system/` | Global system settings (refresh rate, audio, captive portal) |
| **Advanced Tweaks** | `/tweaks/` | RAM Plus, phantom-process limit, power-user toggles |
| **App Manager** | `/apps/` | Bloatware, firewall, permissions audit, per-app details |
| **File Manager** | `/files/` | Browse device storage over ADB |
| **Backup & Restore** | `/backup/` | Encrypted `.ab` backups |
| **Profiles & Snapshots** | `/safety/` | 1-click optimize with atomic undo |
| **Telemetry** | `/telemetry/` | Live process table |
| **Logs & Tools** | `/tools/` | Live logcat/dmesg, bugreport capture, automation export |
| **Toolbox** | `/toolbox/` | Utilities, incl. **Screen Mirror** (scrcpy) |

## Architecture (high level)

```
+--------------------------------+        +-----------------------------------+
|      SvelteKit 2 (Svelte 5)    |  IPC   |            Tauri 2 (Rust)         |
|  Dashboard / Audit / Optimize  | -----> |  ADB client / Parsers / Heuristics|
|  Snapshots / Bloatware / Export| <----- |  Optimizer / Snapshot store / Log |
+--------------------------------+ events +----------------+------------------+
                                                           |  tokio::process
                                                           v
                                                   +---------------+
                                                   |   adb shell   |
                                                   +---------------+
```

### Backend modules (`src-tauri/src/`)

| Module          | Responsibility                                                                 |
|-----------------|--------------------------------------------------------------------------------|
| `adb`           | Async ADB client, device discovery, **multi-device support** (`-s <serial>`), capability probing |
| `parsers`       | **Version-aware** dumpsys / sysfs parsers (batterystats, cpuinfo, alarm, jobscheduler, deviceidle, kernel wakelocks, process status, storage, DNS, ...) with fixtures per API level |
| `heuristics`    | Risk classification, **GMS proxy detection** via `dumpsys alarm`, continuous CPU sampling (p50/p95), bloatware recommendations, hybrid manifest |
| `optimizer`     | Standby buckets, AppOps revocation, selective `am kill`, `pm disable-user --user 0` for bloatware, profiles |
| `snapshot`      | **True differential snapshots** with content-addressed storage, fingerprint-tolerant rollback |
| `export`        | Shell script generator with SHA-256 checksum, MacroDroid task templates |
| `ipc`           | Tauri command handlers + streaming (the only Rust surface exposed to the UI) |
| `security`      | Package/UID guardrails enforced before any destructive action |
| `telemetry`     | Structured logging via `tracing` + rotating file appender |

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full design rationale.

## Setup

### Prerequisites

```powershell
# 1. Node 22+
node --version

# 2. Rust toolchain -- install if missing
winget install Rustlang.Rustup
rustup default stable

# 3. Tauri 2 CLI
cargo install tauri-cli --version "^2.0"

# 4. ADB platform-tools (a copy is also bundled with scrcpy, see below)
adb version
```

### Install + run

```powershell
cd C:\Users\IvN\Documents\GitHub\dozeforge
npm install          # also fetches scrcpy via postinstall (see below)
npm run tauri:dev
```

### Screen Mirror (scrcpy)

The **Screen Mirror** feature bundles [scrcpy](https://github.com/Genymobile/scrcpy).
Its ~40 MB of Windows binaries are **not** committed to git; instead they are
fetched automatically on `npm install` (via `postinstall`). To (re)fetch on demand:

```powershell
npm run setup:scrcpy             # download if missing
npm run setup:scrcpy -- --force  # re-download / upgrade
```

Pinned to `v3.3.4` by default; override with `$env:SCRCPY_VERSION`. See
[`src-tauri/scrcpy/README.md`](src-tauri/scrcpy/README.md) for details and the
manual/offline procedure.

### Build a release bundle

```powershell
npm run tauri:build
```

App icons live in `src-tauri/icons/` and are already committed; see
`src-tauri/icons/README.md` if you want to regenerate them from a single PNG.

## Safety model

DozeForge never:

- Runs as root or asks for root.
- Touches packages with `uid < 10000`.
- Touches anything under `/system`, `/vendor`, or `/apex`.
- Applies a destructive command without a prior snapshot of the affected appops/buckets.
- Restores a snapshot if `sdk_int` differs from the snapshot's `sdk_int`. A change in `security_patch_month` is allowed within the same SDK.

## Repository layout

```
dozeforge/
├─ README.md                  (this file)
├─ LICENSE                    (MIT)
├─ docs/ARCHITECTURE.md       (deep technical rationale)
├─ package.json + svelte/vite/ts configs
├─ scripts/setup-scrcpy.mjs   (fetches the scrcpy Windows build)
├─ src/                       (SvelteKit frontend)
│   ├─ routes/                (Overview, Fleet, Sleep, Battery, Storage, Network,
│   │                          System, Tweaks, Apps, Files, Backup, Safety,
│   │                          Telemetry, Tools, Toolbox)
│   ├─ lib/
│   │   ├─ components/        (DevicePicker, PairingModal, CommandPalette,
│   │   │                      AppDetailsModal, AppName, BatteryHistory,
│   │   │                      DebloatWizard, CapabilitiesBanner, RiskBadge,
│   │   │                      StatCard, Skeleton)
│   │   ├─ stores/            (device, cache, i18n, labels, snapshots, theme,
│   │   │                      appModal — Svelte 5 runes singletons)
│   │   ├─ parsers/           (frontend parsers: appInspector, batteryHistory, trackerScan)
│   │   ├─ data/trackers.ts   (known tracker signatures)
│   │   ├─ tauri/api.ts       (typed wrapper over invoke())
│   │   ├─ types.ts           (mirror of Rust serialised types)
│   │   └─ utils/format.ts
│   └─ styles/global.css
├─ src-tauri/                 (Rust backend)
│   ├─ Cargo.toml             (Tauri 2.1, tokio, tracing, regex, serde, sha2, ...)
│   ├─ tauri.conf.json
│   ├─ capabilities/          (granular Tauri 2 capabilities)
│   ├─ icons/                 (app icons, committed)
│   ├─ scrcpy/                (README only; binaries fetched, git-ignored)
│   ├─ manifests/             (seed hybrid manifest with known offenders)
│   └─ src/
│       ├─ main.rs + lib.rs   (registers the Tauri command surface)
│       ├─ adb/               (client, device, command, capabilities)
│       ├─ parsers/           (dumpsys/sysfs parsers + shared types)
│       ├─ heuristics/        (risk, manifest, proxy_detector, sampling, bloatware)
│       ├─ optimizer/         (actions, executor, bloatware, profiles)
│       ├─ snapshot/          (store, diff, rollback)
│       ├─ export/            (shell_script with SHA-256, macrodroid)
│       ├─ ipc/               (commands + streaming — the exposed surface)
│       ├─ security/          (UID/package guardrails)
│       ├─ telemetry/logger.rs
│       └─ state.rs + error.rs
├─ tests/fixtures/            (batterystats / alarm / jobscheduler for API 34)
└─ .github/workflows/ci.yml   (Rust + Frontend + Android emulator matrix)
```

## License

MIT -- see `LICENSE`.
