# Privacy

DozeForge is a local, offline tool. It is built so that your device data never
leaves your computer.

## What DozeForge does NOT do

- **No telemetry.** There is no usage analytics, crash reporting, or "phone
  home" of any kind. The Rust backend does not even include an HTTP client
  (`reqwest`/`hyper`/etc. are not dependencies), so it has no way to make
  outbound network requests.
- **No accounts, no cloud.** You never sign in. Nothing is uploaded.
- **No third-party trackers** in the app itself. (The tracker *signatures* under
  `src/lib/data/trackers.ts` are a local reference list used to *detect*
  trackers on the phone you're auditing — they are not active in DozeForge.)

## What data is read, and where it stays

- DozeForge reads device state over **ADB** (`dumpsys`, `sysfs`, `pm`,
  `settings`, `top`, etc.) only while a device is connected and only for the
  screen you're viewing. This data is processed in memory and rendered in the
  app; it is not transmitted anywhere.
- **Action log (local only).** Every optimization DozeForge applies — and every
  command run from the in-app ADB console — is appended to a local JSONL audit
  log in the app's data directory on your computer, for traceability and
  rollback. You can view it under *Logs & Tools* and delete it at any time.
- **Snapshots (local only).** Before any reversible change, the prior state
  (appops, standby buckets) is saved to a local, content-addressed snapshot
  store so you can roll back. These files stay on your machine.
- **Settings** (theme, language, the one-time first-run disclaimer flag) are
  stored in the app's local storage.

## Network activity

The app performs **no** network I/O at runtime. The only time anything is
downloaded is at **install/build time**, when `scripts/setup-scrcpy.mjs` fetches
the scrcpy binaries for the Screen Mirror feature, and (optionally, if you run
it yourself) `scripts/sync-uad-list.mjs` updates the bundled bloatware database.
Both are explicit developer/setup steps, not runtime behavior.

## Your device, your control

Everything DozeForge changes is done with public Android primitives and is
reversible. Nothing is sent off-device or off-computer. If you uninstall
DozeForge, deleting its data directory removes the local action log and
snapshots.
