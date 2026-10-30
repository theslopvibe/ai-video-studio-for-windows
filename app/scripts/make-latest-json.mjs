// Usage (from repo root): node app/scripts/make-latest-json.mjs <version> "<release notes>"
// Run after `npm run tauri build` with TAURI_SIGNING_PRIVATE_KEY set. Upload dist/latest.json
// and the installer to the GitHub Release; the app reads releases/latest/download/latest.json.
import fs from "node:fs";
import path from "node:path";
const [version, notes = ""] = process.argv.slice(2);
if (!version) throw new Error("version required");
const dir = "app/src-tauri/target/release/bundle/nsis";
const exe = fs.readdirSync(dir).find((f) => f.includes(version) && f.endsWith("-setup.exe"));
if (!exe) throw new Error("installer for " + version + " not found in " + dir);
const sig = fs.readFileSync(path.join(dir, exe + ".sig"), "utf8").trim();
// GitHub rewrites spaces in asset names, so publish the installer under a hyphenated name.
const asset = `AI-Video-Studio-${version}-setup.exe`;
const url = `https://github.com/theslopvibe/ai-video-studio-for-windows/releases/download/v${version}/${asset}`;
const json = { version, notes, pub_date: new Date().toISOString(), platforms: { "windows-x86_64": { signature: sig, url } } };
fs.mkdirSync("dist", { recursive: true });
fs.copyFileSync(path.join(dir, exe), path.join("dist", asset));
fs.writeFileSync("dist/latest.json", JSON.stringify(json, null, 2));
console.log("wrote dist/latest.json for", exe);
