# Frontend Framework

This file is the catalog of the frontend library in `framework_fe/`.

Each row has the entry path, a short description, and the open behaviour. Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. When a code change would change the open behaviour and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `framework_fe/src/version.ts` | Process version checker | The checker reads `GET /version` on the same origin. It calls `onUpdated` when a later response has a different version. It does not show a UI. The project decides when to reload. |
| `framework_fe/src/http/ajax.ts` | Ajax client | The client adds the prefix `/api` to each URL. A failure throws `ApiError` or `NetworkConnectionException`. The client does not return a result object for that failure. |
| `framework_fe/src/http/exception-handler.ts` | Handler for ajax errors that no code caught | The handler receives only an `ApiError` or a `NetworkConnectionException`. It does not log the error, and it does not stop the error. |
| `framework_fe/src/http/upload.ts`, `framework_fe/src/http/download-file.ts` | File upload and file download | Both functions use the URL as given, with no `/api` prefix. `upload` sends the file as `multipart/form-data`. `downloadFile` saves a 2xx body and throws `NetworkConnectionException` for any other result. |
| `framework_fe/src/storage.ts` | Typed storage | The stored key is `{prefix}:{key}`, and the stored value is JSON. `get` returns `null` when the key is missing, the text is not JSON, or the browser blocks the storage. `set` and `remove` do nothing when the browser blocks the storage. |
| `framework_fe/src/util/sleep.ts` | `sleep` | The promise resolves after the given number of milliseconds. It has no cancellation. |
| `framework_fe/src/solid/ErrorBoundary.tsx` | App error boundary | The project gives the whole fallback. The boundary logs the error and does not pass the error to the fallback. |
| `framework_fe/src/solid/NavigationLogger.tsx` | Navigation logger for the SolidJS router | The component records one info event when the pathname changes. It does not record the first page. It puts the current page URL on the logger context for later events. |
| `framework_fe/src/solid/SwitchRouter.tsx` | Router switch for protected and public routes | The component shows the protected routes when `isProtected` is true. It shows the public routes when `isProtected` is false. A path that is not in the shown routes moves to the path that `unknown` returns for that boolean. The component reads each route list one time, when it is created. |
| `framework_fe/src/logger/` | Browser logger | The logger posts the queue to `POST /log` on the same origin with `sendBeacon`, and it does not add the `/api` prefix. It masks `extra` and `errorMessage` before the event enters the queue. |
| `framework_fe/src/util/time.ts` | Hong Kong time `hktIsoString` | The function writes the date as Hong Kong time with the offset `+08:00`. The browser time zone does not change the value. |
| `framework_fe/src/solid/locale.ts` | Locale helper `createLocale` | The project gives the locales and the dictionaries. `setLocale` does nothing when the new locale has no dictionary. `locale` and `dict` track the current locale. |
