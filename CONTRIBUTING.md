# Contributing to DozeForge

Thanks for your interest! DozeForge is a Tauri 2 + SvelteKit 5 + Rust desktop
app that audits and optimizes Android over ADB. Contributions of all sizes are
welcome — code, docs, and especially **device dumps** (see below).

## The single most useful contribution: a device diagnostic

DozeForge's parsers must handle the many ways OEMs format `dumpsys` output
(Pixel, Nothing, Samsung, Xiaomi…). The best way to help is to send a real dump
from your device so we can harden the parsers and add a test fixture:

1. In the app: **Logs & Tools → Export diagnostic** (one click).
2. **Review the file for anything private** — it contains your installed-app
   list and device fingerprint.
3. Open a **Device compatibility report** issue and attach it.

No device? Even reporting "feature X shows empty on my Samsung Galaxy Sxx" with
the model + Android version is valuable.

## Development setup

Requirements: Node 22+, the Rust stable toolchain, Tauri 2 CLI, and `adb` on
your `PATH` (a copy ships with the bundled scrcpy on Windows).

```bash
npm install          # also fetches scrcpy (postinstall)
npm run tauri:dev    # run the app
```

## Before you open a pull request

Run the same checks CI runs — they must pass:

```bash
npm run check        # svelte-check (types + a11y)
npm test             # TypeScript parser unit tests (vitest)
cargo test           # Rust tests (from src-tauri/)
```

- **Match the existing style.** Rust uses `cargo fmt`; the frontend follows the
  patterns already in `src/`.
- **Parsers must be robust.** Never index a split (`parts[N]`) without a length
  guard, and never `panic!`/`unwrap()` on device output (`panic = abort` in
  release turns any panic into a crash). Prefer `.get(i).unwrap_or(default)`.
- **Any value that reaches `adb shell` must be validated** via
  `src-tauri/src/security/`. Add an anti-injection test for new inputs.
- **New device formats** should come with a fixture and a test.

## Project layout

See the **Project structure** and **Architecture** sections of the [README](README.md).
Backend lives in `src-tauri/src/` (`adb`, `parsers`, `heuristics`, `optimizer`,
`snapshot`, `ipc`, `security`); the SvelteKit frontend in `src/`.

## Licensing of contributions

By contributing, you agree your changes are licensed under the project's
[MIT License](LICENSE). Do not add GPL/AGPL code or data to the bundled binary
(see [NOTICE](NOTICE) for how the community bloatware list is handled as an
optional runtime download).
