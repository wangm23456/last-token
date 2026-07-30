import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";

export const SUPPORTED_LANGUAGES = ["zh-CN", "en"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

const STORAGE_KEY = "last-token.lang";

function detectInitialLanguage(): SupportedLanguage {
  if (typeof window !== "undefined") {
    try {
      const stored = window.localStorage.getItem(STORAGE_KEY);
      if (stored && (SUPPORTED_LANGUAGES as readonly string[]).includes(stored)) {
        return stored as SupportedLanguage;
      }
    } catch {
      // localStorage may be unavailable (private mode, sandboxed iframe); fall through.
    }
    const nav = window.navigator?.language;
    if (nav?.toLowerCase().startsWith("en")) return "en";
  }
  return "zh-CN";
}

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    "zh-CN": { translation: zhCN },
  },
  lng: detectInitialLanguage(),
  fallbackLng: "zh-CN",
  interpolation: { escapeValue: false },
  returnNull: false,
});

export function setLanguage(lang: SupportedLanguage): void {
  void i18n.changeLanguage(lang);
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(STORAGE_KEY, lang);
    } catch {
      // Ignore storage errors; language is already updated in-memory.
    }
  }
}

export default i18n;
