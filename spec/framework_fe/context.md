# Frontend framework context

This document gives the parts that apply to all of the frontend library in `framework_fe/`.
This library is not product business logic.

The feature index is `spec/framework_fe/structure.md`.

## Package

- The package name is `ian-framework-fe`.
- The package path is `framework_fe/`.
- The package has no root entry. A project imports each feature from its own path.
- The package ships TypeScript source. The package has no build step.

## Open behaviour

The open items of this library are in `spec/framework_fe/open-behaviour.json`. Before you change a behaviour, read the open items. The rules for open items are in `spec/framework_rs/context.md` § Open behaviour.
