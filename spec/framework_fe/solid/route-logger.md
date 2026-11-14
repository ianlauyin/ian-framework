# Route logger

This document describes the route logger component in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The source file is `framework_fe/src/solid/route-logger/index.tsx`.
- The component uses SolidJS and the SolidJS router. `solid-js` and `@solidjs/router` are optional peer dependencies of the package.
- A project that imports this component must have `solid-js`, `@solidjs/router` and `vite-plugin-solid`.

## API

`RouteLogger` is a component with no props. It shows nothing.

The project must put the component inside the `Router` of `@solidjs/router`. The component uses `useLocation`, which works only inside the `Router`. Put one component in the root layout, so that the component stays mounted for all the pages.

## Behaviour

The component has two effects. Both use the router location. The logger is in `spec/framework_fe/logger.md`.

### Request URL

- The first effect builds the request URL from `window.location.origin`, then `pathname`, `search` and `hash` of the router location.
- The effect runs when the component mounts, and again each time the URL changes. This includes a change of only the query string or the hash.
- Each run calls `Logger.addContext` with the key `request_url` and the URL after `Logger.maskText`. Thus each later event has the current page URL in its `context`.

### Page switch

- The second effect watches `pathname` of the router location.
- When `pathname` changes, the component records one event with `Logger.info`.
- The component does not record an event for the first page when it mounts.
- A change of only the query string or the hash does not record an event.

The event has these values:

| Field | Value |
| --- | --- |
| `action` | `PAGE/switch` |
| `extra.from` | The `pathname` before the change. |
| `extra.to` | The `pathname` after the change. |

The logger masks `from` and `to`, because they are in `extra`. The masking rules are in `spec/framework_fe/logger.md`.
