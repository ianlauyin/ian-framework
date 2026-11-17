import { createSignal } from "solid-js";

export type Locales<Locale extends string, Dictionary extends object> = Record<Locale, Dictionary>;

export interface LocaleState<Locale extends string, Dictionary extends object> {
  locale(): Locale;
  dict(): Dictionary;
  setLocale(locale: Locale): void;
}

export function createLocale<Locale extends string, Dictionary extends object>(
  locales: Locales<Locale, Dictionary>,
  initial: NoInfer<Locale>,
): LocaleState<Locale, Dictionary> {
  const [current, setCurrent] = createSignal<Locale>(initial);
  return {
    locale: current,
    dict: () => locales[current()],
    setLocale(locale) {
      if (!(locale in locales)) return;
      setCurrent(() => locale);
    },
  };
}
