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
| `framework_fe/src/solid/KeyedRouter.tsx` | Router for keyed route groups, where a list of active keys selects the groups | The router has the routes of each group whose key is in `activeKeys`, in the order of `keyed`. A path that no route of an active group matches goes to `fallbackPath` and replaces the history entry. When `fallbackPath` also has no route of an active group, the router renders nothing. A change of the active groups makes a new router. |
| `framework_fe/src/logger/` | Browser logger | A failed script, style, or image load does not bubble, so the listener uses the capture phase. The logger cannot read a beacon after the browser accepts it. `trackNavigation` wraps `history.pushState` and `history.replaceState`, because the browser sends no event for those calls. It does not use a router. It stores the page URL in the context `requestUrl` and records no event. In that URL, a parameter whose key matches a mask pattern keeps the key and has a masked value. |
| `framework_fe/src/util/time.ts` | Hong Kong time `hktIsoString` | |
| `framework_fe/src/solid/locale.ts` | Locale helper `createLocale` | `setLocale` flushes the store before it returns, so the caller reads the new locale. Solid 2 shows a store write after the batch flushes. |
| `framework_fe/src/solid/App.tsx` | Base class `App` that holds the app store, renders the children in the error boundary, and calls `onStart` | Only a class that extends `App` can write to the store. `store` gives the store to read. `App` has no context, because a component imports the app instance and a store read is reactive in each component. `start` gets the children as a function, because a child that is made before the render is outside the error boundary. `start` does not wait for `onStart`. A failed `onStart` is logged as `APP/start` and does not stop the page. |
| `framework_fe/src/solid/Module.ts` | Base class `Module` that holds a store | Only a class that extends `Module` can write to the store. `store` gives the store to read. `Module` has no context, because a component imports the module instance. |
| `framework_fe/src/solid/PageContext.tsx` | Base class `PageContext` that holds a store and gives its instance to its children through a context | `PageContext` does not extend `Module`. Only a class that extends `PageContext` can read the store and write to the store. The extended class gives each value that a child can read as a public member. Each page instance has its own context, so no page shares a context with a different page. `provide` renders the children in that context. `use` gives the page instance, in a type that does not have `use` and `provide`, and throws when the reader is not in the children of that page. `provide` gets the children as a function, because a child that is made before the call is outside the context. |
