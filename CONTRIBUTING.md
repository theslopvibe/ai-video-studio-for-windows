# Contributing

Thanks for helping. This is a Windows desktop app (Tauri 2, Rust backend, Svelte 5 interface).

## Set up

```
cd app
npm install
npm run dev            # then open http://localhost:1420/?mock for the interface with a fake backend
npm run tauri dev      # the real app (needs Rust stable and the Tauri 2 prerequisites for Windows)
```

## Before you open a pull request

```
cd app/src-tauri && cargo test      # backend tests
cd app && npx svelte-check          # interface type check, must report 0 errors
```

- Write interface text in English and Greek (`app/src/lib/i18n.ts`, `wizard.ts`, `chatStrings.ts`). English comes first. More languages are welcome: copy a key and add the language.
- Backend messages that reach the user (errors, job status) are in English.
- Never log or store secrets outside the Windows Credential Manager (`secrets.rs`).
- A new cloud provider is a Rust module with a way to run a job and test a key, plus workflow JSON files in `app/src-tauri/workflows` (see `hosted.rs`). Hosted modes are plain JSON, so adding a model needs no Rust.
- A new ComfyUI workflow is a JSON file in the same folder (see the existing files for the `meta` block).

## Svelte 5 note

Do not use `bind:value` on a property of a `$derived` value. It froze the chat page once. Use `value={...}` with an `onchange` handler.

## Reporting bugs

Open an issue with: what you did, what you expected, what happened, and the report file from Logs → "Make a report" (it contains no keys).
