// Translations live in locales/<code>.json as flat "namespace.key" -> text maps (namespaces: d, c, w, g).
// English and Greek stay in the source dictionaries; every other language falls back to English per string.
export const LANGS = [
  { code: "en", name: "English" },
  { code: "zh", name: "中文" },
  { code: "hi", name: "हिन्दी" },
  { code: "es", name: "Español" },
  { code: "fr", name: "Français" },
  { code: "ar", name: "العربية", rtl: true },
  { code: "bn", name: "বাংলা" },
  { code: "pt", name: "Português" },
  { code: "ru", name: "Русский" },
  { code: "ur", name: "اردو", rtl: true },
  { code: "el", name: "Ελληνικά" },
] as const;

export type Lang = (typeof LANGS)[number]["code"];

const files = import.meta.glob("./locales/*.json", { eager: true, import: "default" }) as Record<string, Record<string, string>>;
const byLang: Record<string, Record<string, string>> = {};
for (const [p, v] of Object.entries(files)) {
  const code = p.split("/").pop()!.replace(".json", "");
  if (!code.startsWith("_")) byLang[code] = v;
}

/** Translation of `ns.key` for `lang`, or `fallback` (the English/Greek source text) when there is none. */
export function loc(lang: string, id: string, fallback: string): string {
  return byLang[lang]?.[id] || fallback;
}

export const isRtl = (lang: string) => LANGS.some((l) => l.code === lang && "rtl" in l && l.rtl);
