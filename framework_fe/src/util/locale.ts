export type Locales<Locale extends string, Dictionary extends object> = Record<Locale, Dictionary>;

export interface LocaleState<Locale extends string, Dictionary extends object> {
  current(): Locale;
  dictionary(): Dictionary;
  changeLocale(locale: Locale): void;
}

export function createLocale<Locale extends string, Dictionary extends object>(
  locales: Locales<Locale, Dictionary>,
  initial: NoInfer<Locale>,
): LocaleState<Locale, Dictionary> {
  let current = initial;
  return {
    current: () => current,
    dictionary: () => locales[current],
    changeLocale(locale) {
      if (!(locale in locales)) return;
      current = locale;
    },
  };
}
