# Page switch logger

This document describes the page switch logger component in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The source file is `framework_fe/src/solid/page-switch-logger/index.tsx`.
- The component uses SolidJS and the SolidJS router. `solid-js` and `@solidjs/router` are optional peer dependencies of the package.
- A project that imports this component must have `solid-js`, `@solidjs/router` and `vite-plugin-solid`.

## API

`PageSwitchLogger` is a component with no props. It shows nothing.

The project must put the component inside the `Router` of `@solidjs/router`. The component uses `useLocation`, which works only inside the `Router`. Put one component in the root layout, so that the component stays mounted for all the pages.

## Behaviour

- The component watches `pathname` of the router location.
- When `pathname` changes, the component records one event with `Logger.info` from `framework_fe/src/logger/index.ts`. The logger is in `spec/framework_fe/logger.md`.
- The component does not record an event for the first page when it mounts.
- A change of only the query string or the hash does not record an event.

The event has these values:

| Field | Value |
| --- | --- |
| `action` | `PAGE/switch` |
| `context.from` | The `pathname` before the change. |
| `context.to` | The `pathname` after the change. |

The logger masks `from` and `to`, because they are in the call `context`. The masking rules are in `spec/framework_fe/logger.md`.
