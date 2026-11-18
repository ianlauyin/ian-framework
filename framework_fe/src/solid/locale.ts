import { createStore, flush, untrack } from "solid-js";

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
  const [current, setCurrent] = createStore({ locale: initial });
  return {
    locale: () => current.locale,
    dict: () => locales[current.locale],
    setLocale(locale) {
      if (!(locale in locales)) return;
      if (locale === untrack(() => current.locale)) return;
      setCurrent((state) => {
        state.locale = locale;
      });
      flush();
    },
  };
}
