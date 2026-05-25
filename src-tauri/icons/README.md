# Tauri Icons

Tauri's bundler requires platform-specific icon binaries. They are NOT shipped
in the repository because they're easy to regenerate from a single source SVG.

## Quick generation

From any source PNG >= 1024x1024:

```powershell
cd C:\Users\IvN\Documents\GitHub\dozeforge
npm install -g @tauri-apps/cli
cargo tauri icon path\to\source.png
```

This produces:
- `32x32.png`
- `128x128.png`
- `128x128@2x.png`
- `icon.icns` (macOS)
- `icon.ico` (Windows)

## If you only have the SVG

```powershell
# Convert SVG -> PNG with ImageMagick first
magick convert -background none -resize 1024x1024 ..\..\static\favicon.svg source.png
cargo tauri icon source.png
```

## Skipping icons for `tauri dev`

`cargo tauri dev` works without icons present. They are only required by
`cargo tauri build`. Until you generate them, comment out the `icon` array in
`src-tauri/tauri.conf.json` for release builds.
