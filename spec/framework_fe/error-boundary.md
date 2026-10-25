# App error boundary

This document describes the error boundary component in the frontend framework.
This feature is not product business logic.

## Package

The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
The import path is `ian-framework-fe/error-boundary`. The root entry `ian-framework-fe` does not export this component.
The source files are `framework_fe/src/error-boundary/index.tsx` and `framework_fe/src/error-boundary/index.css`.

The component uses SolidJS. `solid-js` is an optional peer dependency of the package. A project that imports this component must have `solid-js` and `vite-plugin-solid`.
The export has the `solid` condition. `vite-plugin-solid` reads this condition, compiles the TSX source and does not pre-bundle the package.

## API

`AppErrorBoundary` is a component. All props are required.

| prop | type | meaning |
| --- | --- | --- |
| `title` | `string` | the heading of the fallback |
| `message` | `string` | the text below the heading |
| `retryLabel` | `string` | the text of the retry button |
| `reloadLabel` | `string` | the text of the reload button |
| `children` | `JSX.Element` | the content that the boundary protects |

The project gives all the text. The framework does not contain text for the user.

## Behaviour

When `children` throws an error, the component shows the fallback in place of `children`.

The fallback has these parts:

- The root element has the id `app-error-boundary` and the class `g-app-error-boundary`.
- The retry button resets the boundary. Then the component shows `children` again.
- The reload button reloads the page.

The fallback fills the viewport height and puts its content in the center.

## Out of scope

This feature does not include these items:

- Error reports or logs.
- A fallback that the project gives as a component.
