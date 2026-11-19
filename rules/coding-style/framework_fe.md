# framework_fe coding style

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Put a decision in `catalog/framework_fe.md`.
- When the text is a reason and not a decision, write a test that prevents that reason.
- Return early when a check fails. Keep the success path at the end of the function.
- Write tests only for functions that hold important logic. For each of these functions, test each edge case. Do not test a trivial function, a getter or a pass-through.
- Name each test by the behavior that it checks.
- After you change the frontend code, run `pnpm typecheck` and `pnpm lint` in `framework_fe/`. Fix all the errors that they show. The Biome formatter is off.

## Files

- Put each library file in `framework_fe/src/`.

## Modules

- The index re-exports the items that a project imports.
- An export for a sibling file or a test can stay out of the index.
- Import a type with the `type` keyword.

## Errors

- A type that matches a JSON body from the Rust library uses the same field names as that body.
- A path parameter check throws `Error` before the request.
- An HTTP failure throws `ApiError` or `NetworkConnectionException` from `framework_fe/src/http/network.ts`. Do not return a result object for that failure.
