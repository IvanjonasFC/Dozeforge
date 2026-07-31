# Security Policy

DozeForge is a desktop tool that talks to Android devices over ADB. Its core
optimizer is no-root and reversible, but the app also ships advanced tools
(Recovery / Root) that can flash partitions, switch A/B slots, set SELinux
permissive and write to the kernel. Because of that, we take security and
correctness seriously.

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Use GitHub's private vulnerability reporting:
**Security tab → "Report a vulnerability"** on this repository. This keeps the
report confidential until a fix is available.

Please include:

- A description of the issue and its impact.
- Steps to reproduce (device model / ROM / Android version if relevant).
- Any logs or the output of **Logs & Tools → Export diagnostic** (review it for
  personal data first — it contains your installed-app list).

We aim to acknowledge reports within a few days.

## Scope

In scope:

- **Command injection** into `adb shell` via any value that crosses the IPC
  boundary (serials, package names, ops, paths). All such input is validated in
  `src-tauri/src/security/`.
- **XSS / code execution** in the WebView (e.g. rendering untrusted `dumpsys`
  output as HTML) that could reach the unrestricted `run_shell` console.
- **Destructive-action safety**: any path that can modify a device without a
  snapshot, touch `uid < 10000` or `/system|/vendor|/apex`, or restore a
  snapshot across an SDK major change.
- Tauri capability / CSP misconfiguration that widens the attack surface.

Out of scope:

- Bricking a device by **deliberately** using the Recovery/Root tools as
  documented. These are opt-in and gated; misuse is the user's responsibility
  (see the disclaimer in the README).
- Issues in third-party binaries we don't ship (system `adb`/`fastboot`).

## Supported versions

DozeForge is pre-1.0 (beta). Only the latest release / `main` receives fixes.
