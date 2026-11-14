# Frontend framework context

This document gives the parts that apply to all of the frontend library in `framework_fe/`.
This library is not product business logic.

The feature index is `spec/catalog.md`.

## Package

- The package name is `ian-framework-fe`.
- The package path is `framework_fe/`.
- The package has one entry, `ian-framework-fe`. The root entry is `framework_fe/src/index.ts`. It re-exports each feature, the Solid components included.
- A project that imports the root entry must have `solid-js` and `@solidjs/router`, because the root entry also loads the Solid components.
- The package ships TypeScript source. The package has no build step.
