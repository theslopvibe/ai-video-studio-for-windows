# End-to-end test (no Vast keys needed)

`mock.mjs` imitates Vast (`/route/`, worker `/generate/sync`, console API). `e2e.mjs` launches the release exe with
`AIVS_VAST_RUN_URL` / `AIVS_VAST_CONSOLE_URL` pointing at the mock and drives it through WebView2 DevTools.

```
ffmpeg -y -f lavfi -i testsrc=duration=1:size=320x192:rate=16 -c:v libx264 -pix_fmt yuv420p out/mock.mp4
MOCK_MP4=out/mock.mp4 node mock.mjs &
E2E_DIR=<abs out dir> APP_EXE=<abs path to app.exe> node e2e.mjs
```
It uses the real `%APPDATA%\com.aivideostudio.app` database and removes the fake Vast key from Credential Manager at the end.
