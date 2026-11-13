# Small helpers

This document describes the small helpers in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- `ian-framework-fe` exports each helper. Each helper has its own source file in `framework_fe/src/`.
- `framework_fe/src/util/index.ts` re-exports the helpers in `framework_fe/src/util/`, and the root entry re-exports it.
- Only `createStorage` has a test, in `framework_fe/test/storage.test.ts`. `sleep` is trivial.

| Source | Item | Meaning |
| --- | --- | --- |
| `framework_fe/src/storage.ts` | `createStorage<Schema>(prefix, area?)` | Typed JSON values in `localStorage` (default) or `sessionStorage` (`"session"`). |
| `framework_fe/src/util/sleep.ts` | `sleep(ms)` | A promise that resolves after `ms` milliseconds. |

## Storage

- The storage has `get(key)`, `set(key, value)` and `remove(key)`. The TypeScript type of `Schema` gives the key names and the value types.
- The stored key is `{prefix}:{key}`. The stored value is JSON text.
- `get` returns `null` when the key is not there, when the text is not valid JSON, or when the browser blocks the storage.
- `set` and `remove` do nothing when the browser blocks the storage or the storage is full. They do not throw.
