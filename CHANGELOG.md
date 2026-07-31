# Changelog

All notable changes to DozeForge are documented here. This project adheres to
[Semantic Versioning](https://semver.org/) and
[Keep a Changelog](https://keepachangelog.com/).

## [1.0.0-beta.1] — 2026-07-31

First public beta.

### Added
- No-root, reversible battery/CPU/storage optimizer for Android 12+ over ADB.
- Per-app battery drain with verdicts (zombie / background hog / radio hog /
  media / foreground), battery health, cycles and capacity.
- Sleep analyzer: Doze state machine, standby buckets manager (colour-coded),
  kernel & app wakelocks with actionable quick-fixes, miscategorized apps.
- Storage: reclaimable app cache and full category breakdown (dual-format
  `diskstats` parsing).
- Debloat: risk-tiered recommendations with a bundled MIT seed and an optional,
  user-downloaded UAD-NG community overlay.
- Diagnostics: thermal, per-app network, offline tracker scan, APK inventory.
- Toolbox: audited ADB console, logcat, scrcpy screen mirror, fleet actions,
  recovery/fastboot tools, encrypted backup/restore.
- **Logs & Tools → Export diagnostic**: one-click device dump to help expand
  device coverage.
- Cross-platform builds (Windows/macOS/Linux) via GitHub Actions.

### Security / privacy
- 100% local: no telemetry, no analytics, no network egress (the backend has no
  HTTP client). See [PRIVACY.md](PRIVACY.md).
- Every value crossing the IPC boundary into `adb shell` is validated
  (anti-injection tests); the WebView CSP is locked down.
- First-run safety disclaimer for the advanced Recovery/Root tools.

### Robustness (multi-device)
- Layered, fallback-based readers: battery (dumpsys → sysfs → batterystats),
  thermal (thermalservice → `/sys/class/thermal`), storage (diskstats → `df`).
- Vendor-tolerant parsers (Pixel, Nothing, Samsung, Xiaomi) with real-device
  fixtures; graceful "not exposed" states instead of crashes.

[1.0.0-beta.1]: https://github.com/IvanjonasFC/Dozeforge/releases/tag/v1.0.0-beta.1
