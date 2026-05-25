# DozeForge — Architecture

> Detailed design rationale. Read `README.md` first for the user-facing summary.

## 1. Process model

DozeForge runs as a single Tauri 2 process with two logical halves:

- **Native (Rust)**: owns the ADB connection pool, parsers, heuristics,
  snapshot store, and the IPC surface. Single-binary, ~6 MB stripped.
- **Web (SvelteKit 2 -> static SPA)**: pure presentation. Talks to native via
  the typed `api` wrapper in `src/lib/tauri/api.ts`. No network I/O -- even
  manifest updates flow through the native side.

There is no second backend, no sidecar binary, no IPC bus beyond the Tauri
command channel.

## 2. ADB layer

```
AdbClient::discover  <-- locates `adb` via PATH or ANDROID_HOME
        |
        v
AdbInvoker          <-- wraps `tokio::process::Command` with timeouts
        |
        v
AdbClient methods   <-- high-level: list_devices, build_identity, ...
```

Every call requires an explicit `&DeviceSerial`. There is no implicit
"current device" anywhere in the codebase. This is what allows multi-device
support to be a first-class concept rather than a hack.

Timeouts are mandatory. `DEFAULT_TIMEOUT = 30s`, `TOP_TIMEOUT = 60s`. The
`tokio::time::timeout` wrapper makes a hung ADB call recoverable instead of
permanently stalling the UI.

## 3. Parser stack

All parsers implement the `Parser<Output = T>` trait. They are deterministic,
synchronous, and unit-tested against fixtures stored in `tests/fixtures/`.

Format drift between API levels is small in practice but real:
- API 31 introduces the phantom-process killer (new `dumpsys jobscheduler` flags)
- API 33 added `bg_count` to `pwl` rows in batterystats
- API 34+ stable

We version the parsers via constructor (`BatteryStatsParser::for_api(34)`).
The constructor returns the same struct today, but the indirection lets us
swap implementations without changing call sites.

## 4. Heuristics — the GMS-proxy problem

The single most useful insight DozeForge encodes is the **proxy redirection**.
Naïve auditors say:

> Your battery is being drained by `com.google.android.gms` (1.8h wakelock)

The user can't act on that. GMS can't be disabled, restricted, or removed
without bricking half the OS. The information is technically true and
operationally useless.

What's actually happening: GMS is a router. Third-party apps schedule alarms
that target a GMS receiver (e.g. `gcm.nts.TaskExecutionService`). When the
alarm fires, GMS dispatches it back to the third-party process. The wakelock
is owned by GMS but **caused by the third-party app**.

The fix:

```
For each wakelock entry where package in PROXY_PACKAGES:
  Find the third-party package that scheduled the most alarms
  targeting this proxy. Reassign the wakelock attribution to that
  package. Mark `redirected_from_proxy` so the UI can show the chain.
```

This is implemented in `heuristics::proxy_detector::rank()`.

## 5. CPU sampling

Single `top` invocations are misleading because Android processes are bursty.
A push receiver might be at 0% CPU for 28 of the 30 sampled seconds and at
80% for the other 2. `top -n 1` gives you either 0% or 80% -- no useful signal.

`CpuSampler` runs `top -b -n 1` repeatedly (default: 2-second interval x 15
samples -> 30-second window). Per-PID arrays are accumulated, then we compute
p50, p95, and max. p95 surfaces the offenders that single snapshots miss.

The sampling configuration is exposed in the UI; high-variance investigations
can extend to 120 seconds.

## 6. Snapshot store

Snapshots are JSON files content-addressed by SHA-256 of their serialised
form. Location: `<app_data>/snapshots/<sha256>.json`.

A snapshot captures:
- `BuildIdentity` (SDK + security patch)
- `device_serial`
- For each affected package: `appops` + `standby_bucket`

Critically: **a snapshot only captures the packages affected by the action
that triggered it**, not the whole device. This makes:
- snapshots fast to create
- diffs meaningful
- rollback granular

## 7. Rollback safety

`Rollback::execute` refuses if `snapshot.identity.sdk_int != live.sdk_int`.
The reason: an SDK upgrade can change the meaning of bucket values, deprecate
appops, or introduce phantom-process behaviour. Replaying a snapshot blindly
across a major upgrade is a way to soft-brick a device.

A change in the **security patch month** is logged at WARN but allowed. The
appops/standby APIs are stable within an SDK level even when the patch
changes.

## 8. Export integrity

Exported shell scripts carry a SHA-256 header of their body. The self-check
preamble recomputes the hash at runtime and aborts with exit 78 if it does
not match -- preventing accidental corruption when the script is shuttled
across devices through Termux/Shizuku.

## 9. Logging

`tracing` + `tracing-subscriber` with two sinks:
- stderr (always)
- JSON file in `<app_data>/logs/dozeforge.log.YYYY-MM-DD` (opt-out via
  `DOZEFORGE_NO_LOG=1`)

Every ADB command is logged at DEBUG with its arguments. No package contents
are ever logged -- only command lines and aggregate counters.

## 10. Frontend contract

The frontend talks to the backend through exactly the 15 functions defined
in `src/lib/tauri/api.ts`. Each one maps 1:1 to a Tauri command. New
capabilities require:

1. A new Rust function in `src-tauri/src/ipc/commands.rs`
2. Registration in `src-tauri/src/lib.rs::run()`
3. A new method on the `api` object
4. Updated types in `src/lib/types.ts`

The contract is enforced by review, not by codegen. The types are small
enough that the duplication cost is lower than the codegen cost.
