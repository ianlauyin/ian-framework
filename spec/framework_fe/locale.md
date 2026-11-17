# Locale helper

This document describes the locale helper in the frontend framework.
This feature is not product business logic. The project gives the locales and the dictionaries.

## Package

- The import path is `ian-framework-fe`.
- The source file is `framework_fe/src/util/locale.ts`.
- The test file is `framework_fe/test/util/locale.test.ts`.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `Locales<Locale, Dictionary>` | type | A `Record` from each locale to its dictionary. |
| `createLocale(locales, initial)` | function | Makes a `LocaleState` that starts with the locale `initial`. |
| `LocaleState.current()` | method | Gives the current locale. |
| `LocaleState.dictionary()` | method | Gives the dictionary of the current locale. |
| `LocaleState.changeLocale(locale)` | method | Changes the current locale. |

## Behaviour

- `changeLocale` does nothing when `locales` has no dictionary for the new locale.
- The state is not reactive. A project that needs reactivity keeps its own signal or store and calls `changeLocale` when the value changes.
