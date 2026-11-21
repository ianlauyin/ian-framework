# Frontend Framework

This file is the catalog of the frontend library in `framework_fe/`.

- The decision records a behaviour that the code does not show. 
- Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. 
- When a code change would change the decision and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `framework_fe/src/version.ts` | Process version checker | `reload` fetches the current page so the browser does not reuse a cached page that still names the old assets. |
| `framework_fe/src/http/ajax.ts`, `framework_fe/src/http/raw-send.ts`, `framework_fe/src/http/network.ts` | Ajax client | A path parameter `.` or `..` is rejected because encoding leaves those texts unchanged, and the browser then changes the path. Status 0, 502, and 504 throw `NetworkConnectionException`, because a gateway or a proxy uses those statuses when the app is not reached. Status 503 throws `ApiError`, because the app uses 503 for a planned stop. |
| `framework_fe/src/http/exception-handler.ts` | Handler for ajax errors that no code caught | |
| `framework_fe/src/http/upload.ts`, `framework_fe/src/http/download-file.ts` | File upload and file download | |
| `framework_fe/src/storage.ts` | Typed storage | |
| `framework_fe/src/util/sleep.ts` | `sleep` | |
| `framework_fe/src/solid/ErrorBoundary.tsx` | App error boundary | |
| `framework_fe/src/solid/NavigationLogger.tsx` | Navigation logger for the SolidJS router | |
| `framework_fe/src/solid/AccessRouter.tsx` | Router for a route list where each top-level route can have a `canAccess` check | A route with no `canAccess` is open. A route whose `canAccess` returns false is not in the router, so a later route with the same path can match. A path that no open route matches goes to `fallbackPath` and replaces the history entry. When `fallbackPath` also has no open route, the router renders nothing. A change of a `canAccess` result makes a new router. `canAccess` on a child route has no effect. |
| `framework_fe/src/logger/` | Browser logger | A failed script, style, or image load does not bubble, so the listener uses the capture phase. The logger cannot read a beacon after the browser accepts it. |
| `framework_fe/src/util/time.ts` | Hong Kong time `hktIsoString` | |
| `framework_fe/src/solid/locale.ts` | Locale helper `createLocale` | `setLocale` flushes the store before it returns, so the caller reads the new locale. Solid 2 shows a store write after the batch flushes. |
