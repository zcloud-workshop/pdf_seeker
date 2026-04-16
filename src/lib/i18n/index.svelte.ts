import en from "./locales/en.json";
import zh from "./locales/zh.json";

const messages: Record<string, Record<string, unknown>> = { en, zh };

// $state works in .svelte.ts — shared reactive state across components
let locale = $state("zh");

function getNested(obj: unknown, path: string): string {
  return path
    .split(".")
    .reduce(
      (acc: unknown, key: string) =>
        acc && typeof acc === "object" ? (acc as Record<string, unknown>)[key] : undefined,
      obj,
    ) as string;
}

export function t(key: string): string {
  const dict = messages[locale] ?? messages["en"];
  return (getNested(dict, key) as string) ?? key;
}

export function setLocale(l: string) {
  locale = l;
}

export function getLocale(): string {
  return locale;
}
