# App error boundary

This document describes the error boundary component in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The source file is `framework_fe/src/solid/error-boundary/index.tsx`.
- The component uses SolidJS. `solid-js` is an optional peer dependency of the package.
- A project that imports this component must have `solid-js` and `vite-plugin-solid`.

## API

`ErrorBoundary` is a component that wraps the `ErrorBoundary` of `solid-js`. All props are necessary.

| Prop | Type | Meaning |
| --- | --- | --- |
| `fallback` | `(reset: () => void) => JSX.Element` | The function that the project gives. The boundary shows its result in place of `children` when `children` throws. `reset` resets the boundary. Then the boundary shows `children` again. |
| `children` | `JSX.Element` | The content that the boundary protects. |

The fallback function does not get the error. The boundary logs the error. The project owns the whole fallback: the markup, the style and the text for the user. The framework has no fallback markup, no CSS and no text for the user.

## Behaviour

When `children` throws an error, the component logs the error and shows `fallback` in place of `children`.

### Error log

The component logs the error with `Logger.error` from `framework_fe/src/logger/index.ts` before it shows `fallback`. The logger is in `spec/framework_fe/logger.md`.

- The log message is `ERROR-BOUNDARY/caughtError`.
- The log `error_message` is the `message` of the error, or the thrown value as text. The message is only in `error_message`. The `context` has no `error_message` key. The component does not choose a fallback text. When that text is empty or has only whitespace, `Logger.error` gives the fallback text `unknown error`. The rule is in `spec/framework_fe/logger.md`.
- The log has the `context` fields in the table below.
- Each line break in these values becomes ` | `. The reason is that the log route drops a value with a line break. The log route is in `spec/framework_rs/logging/client-log.md`.

| Field | Value | When |
| --- | --- | --- |
| `error_name` | `name` of the error. | Only when the thrown value is an `Error`. |
| `error_stack` | `stack` of the error, or an empty string. | Only when the thrown value is an `Error`. |
