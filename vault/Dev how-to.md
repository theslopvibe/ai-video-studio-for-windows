---
tags: [ai-video-studio, dev]
---
# Dev how-to

```
cd E:\claude\video-generator\app
npm install
npm run check                 # svelte-check (0 errors expected)
npm run tauri dev             # dev run (vite :1420)
npm run tauri build           # release + NSIS → src-tauri/target/release/bundle/nsis/
cd src-tauri && cargo test    # unit tests
```
Installer copies: `dist/` and Desktop. Version in `tauri.conf.json` + `Cargo.toml` + `package.json` (0.1.0).

## Paths
- Code: `app/src-tauri/src/*.rs`, `app/src/lib/*.svelte`, workflows `app/src-tauri/workflows/`, catalog `models.json`, node `aivs_save_b64.py`.
- App data: `%APPDATA%\com.aivideostudio.app\` (`studio.db`, `workflows\`). Default videos: `%USERPROFILE%\Videos\AI Video Studio\`.
- Docs scratch (may be gone): Vast docs/SDK copies were downloaded to the session scratchpad only.

## Pitfalls learnt
- Windows `claude` is `claude.cmd` (npm shim): Rust `Command` runs it fine, but locate via PATH/APPDATA\npm (`anthropic::locate_cli`). Use `CREATE_NO_WINDOW`.
- Bash heredocs with Python containing quotes/`'\n'` get mangled → write patch scripts to files, run `python file.py`. Use `python -I` only for untrusted/isolated, Pillow lives in user site (no `-I`).
- Don't `taskkill /IM node.exe` (kills Claude Code itself; the harness blocks it). Kill the mock by port (`Get-NetTCPConnection -LocalPort 18777`) and the app by exe path.
- Don't use `tauri icon` output android/ios folders (deleted).
- Release build ≈ 2-3 min; run in background or with long timeout.
- Svelte 5: no `$state` writes inside templates; `@render` snippets for repeated blocks.
