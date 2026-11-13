# Exception handler

This document describes the handler for the uncaught ajax errors in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe/exception-handler`.
- The source file is `framework_fe/src/http/exception-handler.ts`.
- The test file is `framework_fe/test/http/exception-handler.test.ts`.
- The handler uses only browser APIs.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `setupExceptionHandler(handler)` | function | Gives each uncaught `ApiError` and `NetworkConnectionException` to `handler`. |
| `ExceptionHandler` | type | `(exception: ApiError \| NetworkConnectionException) => void \| Promise<void>`. |

The two error classes are in `spec/framework_fe/ajax.md`.

## Behaviour

- The function adds one `error` listener and one `unhandledrejection` listener on `window`.
- The handler gets `event.error` or `event.reason` only when it is an `ApiError` or a `NetworkConnectionException`. The handler does not get other errors. `Logger.watchWindowErrors` records them, as in `spec/framework_fe/logger.md`.
- While the handler runs, the function ignores each new error.
- A second call throws an `Error`.
- The function does not log the error and does not stop it. The browser console still shows it.
