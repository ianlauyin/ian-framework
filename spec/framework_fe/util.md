# Small helpers

This document describes the small helpers in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- Each helper has its own import path and its own source file in `framework_fe/src/`.
- Only `createStorage` has a test, in `framework_fe/test/util/storage.test.ts`. `sleep` is trivial.

| Import path | Item | Meaning |
| --- | --- | --- |
| `ian-framework-fe/storage` | `createStorage<Schema>(prefix, area?)` | Typed JSON values in `localStorage` (default) or `sessionStorage` (`"session"`). |
| `ian-framework-fe/sleep` | `sleep(ms)` | A promise that resolves after `ms` milliseconds. |

## Storage

- The storage has `get(key)`, `set(key, value)` and `remove(key)`. The TypeScript type of `Schema` gives the key names and the value types.
- The stored key is `{prefix}:{key}`. The stored value is JSON text.
- `get` returns `null` when the key is not there, when the text is not valid JSON, or when the browser blocks the storage.
- `set` and `remove` do nothing when the browser blocks the storage or the storage is full. They do not throw.
