// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;

// Dev only: `npm run dev` and open http://localhost:1420/?mock to work on the UI without the Rust backend.
export const load = async () => {
  if (import.meta.env.DEV && typeof location !== "undefined" && new URLSearchParams(location.search).has("mock")) {
    (await import("$lib/mock")).installMock();
  }
  return {};
};
