# DozeForge

> Forge your own power-management rules for Android. Surgical battery and CPU auditor over ADB, with atomic rollback and zero root.

**Status:** Pre-alpha · **Target:** Android 12+ (API 31..35) · **Stack:** Tauri 2 · Rust 1.79+ · SvelteKit 2 · Svelte 5

---

## Why DozeForge

Modern Android leaks battery not because of "bad apps" you can guess from a blocklist, but because of:

1. **OEM bloatware** that survives every reboot and ignores Doze (Samsung Health, MIUI Cleaner, Xiaomi GetApps...).
2. **Third-party apps proxying through Google Play Services** (FCM, JobScheduler) so the symptom shows up as `com.google.android.gms` while the culprit is hidden.
3. **Phantom-process killer (API 31+)** that nukes legitimate background tools (Termux, Tasker) while OEM background services keep running fine.

DozeForge reads the actual on-device telemetry over ADB, traces the real culprit (not the proxy), applies progressive restrictions using only public `cmd appops` / `am set-standby-bucket` / `pm disable-user` primitives, snapshots the prior state, and lets you roll back atomically -- or export everything as a Termux/Shizuku shell script so the rules survive without ever needing the PC again.

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
| `parsers`       | **Version-aware** dumpsys parsers (batterystats, cpuinfo, alarm, jobscheduler, deviceidle, power, sensorservice, standby, appops, packages) with fixtures per API level |
| `heuristics`    | Risk classification, **GMS proxy detection** via `dumpsys alarm`, continuous CPU sampling (p50/p95), hybrid manifest |
| `optimizer`     | Standby buckets, AppOps revocation, selective `am kill`, `pm disable-user --user 0` for bloatware |
| `snapshot`      | **True differential snapshots** with content-addressed storage, fingerprint-tolerant rollback |
| `export`        | Shell script generator with SHA-256 checksum, MacroDroid task templates |
| `ipc`           | Tauri command handlers (the only Rust surface exposed to the UI) |
| `telemetry`     | Structured logging via `tracing` + rotating file appender |

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full design rationale.

## Roadmap of weaknesses fixed vs. the original PDF

| # | Weakness from PDF                                       | Status in DozeForge                                                   |
|---|---------------------------------------------------------|-----------------------------------------------------------------------|
| 1 | Angular overkill                                        | Done. Replaced by SvelteKit 2 + Svelte 5                              |
| 2 | Regex parsers break across API levels                   | Done. `Parser` trait + `BatteryStatsParser::for_api(level)` + fixtures |
| 3 | `top -b -n 1` is a single snapshot                      | Done. `CpuSampler` (continuous, p50/p95 over 30s window)              |
| 4 | Wakeup detection only via `dumpsys alarm`               | Done. Combined alarm + jobscheduler + deviceidle + power + sensorservice |
| 5 | `pm disable-user` not in PDF                            | Done. First-class `BloatwareManager` (reversible via `pm enable`)     |
| 6 | No capability detection (MIUI/OneUI reject some appops) | Done. `CapabilityProbe` runs at device-attach, downgrades gracefully  |
| 7 | `am kill-all` too coarse                                | Done. Selective `am kill <pkg>` driven by risk classifier             |
| 8 | `ro.build.fingerprint` too strict for rollback          | Done. `BuildIdentity { sdk_int, security_patch_month }` with tolerance |
| 9 | Multi-device not contemplated                           | Done. Every command takes `&DeviceSerial`; no implicit device         |
| 10| Threading not defined                                   | Done. Async tokio + per-device dispatch                               |
| 11| Snapshots called "differential" but were absolute       | Done. Real diffs against last snapshot, content-hashed                |
| 12| Manifest update mechanism missing                       | Done. Pull from `manifests/packages.json` (signing planned)           |
| 13| Exported `.sh` lacks integrity                          | Done. SHA-256 header + self-verify on execution                       |
| 14| No structured logging                                   | Done. `tracing` + JSON file sink, opt-in only                         |
| 15| Rollback only global                                    | Done. Per-package and per-session rollback                            |
| 16| `max_phantom_processes = INT_MAX` dangerous default     | Done. Configurable preset (64/128/256/1024) with safety cap           |
| 17| No E2E tests                                            | Done. GitHub Actions matrix with `reactivecircus/android-emulator-runner` |

## Setup

### Prerequisites

```powershell
# 1. Node 22+ (you already have v22.22.2)
node --version

# 2. Rust toolchain -- install if missing
winget install Rustlang.Rustup
rustup default stable

# 3. Tauri 2 CLI
cargo install tauri-cli --version "^2.0"

# 4. ADB platform-tools (you already have 1.0.41)
adb version
```

### Install + run

```powershell
cd C:\Users\IvN\Documents\GitHub\dozeforge
npm install
cargo tauri dev
```

### Build a release bundle

```powershell
cargo tauri build
```

Icons are not bundled in the repo; see `src-tauri/icons/README.md` for how to
generate them from a single PNG before running `cargo tauri build`.

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
+- README.md                  (this file)
+- LICENSE                    (MIT)
+- docs/ARCHITECTURE.md       (deep technical rationale)
+- package.json + svelte/vite/ts configs
+- src/                       (SvelteKit frontend)
|   +- routes/                (Dashboard, Audit, Optimize, Snapshots, Bloatware, Settings)
|   +- lib/
|       +- components/        (DevicePicker, CulpritTable, OptimizationPanel, RiskBadge, CapabilitiesBanner)
|       +- stores/            (device, audit, snapshots -- Svelte 5 runes singletons)
|       +- tauri/api.ts       (typed wrapper over invoke())
|       +- types.ts           (mirror of Rust serialised types)
|       +- utils/format.ts
+- src-tauri/                 (Rust backend)
|   +- Cargo.toml             (Tauri 2.1, tokio, tracing, regex, serde, sha2, ...)
|   +- tauri.conf.json
|   +- capabilities/main.json (granular Tauri 2 capabilities)
|   +- manifests/packages.json (seed hybrid manifest with known offenders)
|   +- src/
|   |   +- main.rs + lib.rs   (registers 15 Tauri commands)
|   |   +- adb/               (client, device, command, capabilities)
|   |   +- parsers/           (10 parsers + shared types)
|   |   +- heuristics/        (risk, manifest, proxy_detector, sampling)
|   |   +- optimizer/         (actions, executor, bloatware)
|   |   +- snapshot/          (store, diff, rollback)
|   |   +- export/            (shell_script with SHA-256, macrodroid)
|   |   +- ipc/commands.rs    (the 15 commands)
|   |   +- telemetry/logger.rs
|   |   +- state.rs + error.rs
|   +- tests/integration.rs   (parsers + heuristics against fixtures)
+- tests/fixtures/            (batterystats / alarm / jobscheduler for API 34)
+- .github/workflows/ci.yml   (Rust + Frontend + Android emulator matrix)
+- static/favicon.svg
```

## License

MIT -- see `LICENSE`.
