# Time helpers

This document describes the time and time zone helpers in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe/time`.
- The source file is `framework_fe/src/time.ts`.
- The test file is `framework_fe/test/time.test.ts`.
- The helpers use only the JavaScript standard library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `hktIsoString(date)` | function | Gives `date` as Hong Kong time (HKT) ISO 8601 text with milliseconds and the offset `+08:00`. |

## Behaviour

- HKT is the fixed offset UTC+08:00. HKT has no daylight saving time.
- The browser time zone does not change the value.
- The browser logger uses `hktIsoString` for the event `ts`. The logger is in `spec/framework_fe/logger.md`.
