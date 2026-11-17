# Locale helper

This document describes the locale helper in the frontend framework.
This feature is not product business logic. The project gives the locales and the dictionaries.

## Package

- The import path is `ian-framework-fe`.
- The source file is `framework_fe/src/solid/locale.ts`.
- The test file is `framework_fe/test/solid/locale.test.ts`.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `Locales<Locale, Dictionary>` | type | A `Record` from each locale to its dictionary. |
| `createLocale(locales, initial)` | function | Makes a `LocaleState` that starts with the locale `initial`. |
| `LocaleState.locale()` | method | Gives the current locale. |
| `LocaleState.dict()` | method | Gives the dictionary of the current locale. |
| `LocaleState.setLocale(locale)` | method | Changes the current locale. |

## Behaviour

- `setLocale` does nothing when `locales` has no dictionary for the new locale.
- The state is reactive. The current locale is a Solid signal, so `locale()` and `dict()` track the locale inside an effect or a component.
- The helper needs `solid-js`, which is an optional peer dependency of the package.
