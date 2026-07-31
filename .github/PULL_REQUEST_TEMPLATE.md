<!-- Thanks for contributing to DozeForge! -->

## What does this PR do?

<!-- A short description of the change and the motivation. Link any issue: Closes #123 -->

## Type

- [ ] Bug fix
- [ ] New feature
- [ ] Parser / device-format robustness
- [ ] Docs
- [ ] Refactor / chore

## Checklist

- [ ] `npm run check` passes (svelte-check)
- [ ] `npm test` passes (vitest)
- [ ] `cargo test` passes (from `src-tauri/`)
- [ ] No `panic!`/`unwrap()` on device output; splits are length-guarded
- [ ] Any new value reaching `adb shell` is validated in `security/` (with a test)
- [ ] New device format includes a fixture + test
- [ ] No GPL/AGPL code or data added to the bundled binary

## Testing

<!-- How did you test this? Device model / ROM / Android version if relevant. -->
