# Process version checker

This document describes the version checker in the frontend framework.
This feature is not product business logic.

## Package

The package name is `ian-framework-fe`.
The package path is `framework_fe/`.
The import paths are `ian-framework-fe` and `ian-framework-fe/version`.
The entry file is `framework_fe/src/index.ts`. It re-exports `VersionChecker` and `VersionCheckerOptions`.
The source file is `framework_fe/src/version.ts`.
The test file is `framework_fe/test/version.test.ts`.

The package ships TypeScript source. The package has no build step.
The project bundler and the project TypeScript compiler read the source.
The checker uses only browser APIs. The checker does not use a UI library.

A project adds the package as a git dependency with a subdirectory. Put this line in the project `package.json`:

```json
"ian-framework-fe": "github:ianlauyin/ian-framework#<commit>&path:/framework_fe"
```

Use these commands in `framework_fe/`:

```sh
pnpm install
pnpm typecheck
pnpm test
```

## API

`VersionChecker` is a class with static methods.

| method | effect |
| --- | --- |
| `VersionChecker.init(options)` | starts the checker |

`options` is a `VersionCheckerOptions` object:

| field | type | meaning |
| --- | --- | --- |
| `onUpdated` | `() => void` | required. The checker calls it when the version changes |
| `checkInterval` | `number` | optional. The time between timer checks, in milliseconds. If it is not set, the checker does not use a timer |

The project calls `init()` one time when the page starts.
A second call to `init()` has no effect. The checker ignores the options of the second call.

## Behaviour

The checker reads `GET /version` on the same origin with `credentials: "same-origin"`.
The route is `spec/framework_rs/version.md`.

The checker reads the route at these times:

- When `init()` runs.
- When the page becomes visible. The checker uses the `visibilitychange` event.
- Each `checkInterval` milliseconds, if `checkInterval` is set. The timer does not depend on the page visibility.

The first good response gives the current version.
When a later response gives a different version, the checker calls `onUpdated`.
The checker does not replace the current version. Each later different response calls `onUpdated` again.

The checker ignores these failures:

- A network error.
- A response status that is not 2xx.
- A body that is not JSON.

## Out of scope

This feature does not include these items:

- The UI that tells the user about the new version.
- A way to stop the checker.
