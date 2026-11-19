 # framework_fe coding style

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Do not write a comment in the code. Put a decision in `catalog/framework_fe.md`.
- When the text is a reason and not a decision, write a test that prevents that reason.
- Return early when a check fails. Keep the success path at the end of the function.
- Write tests only for functions that hold important logic. For each of these functions, test each edge case. Do not test a trivial function, a getter or a pass-through.
- Name each test by the behavior that it checks.
- After you change the frontend code, run `pnpm typecheck` and `pnpm lint` in `framework_fe/`. Fix all the errors that they show. The Biome formatter is off.

## Files

- Put each library file in `framework_fe/src/`. A component with styles uses `framework_fe/src/{name}/index.tsx` and `framework_fe/src/{name}/index.css`.
- In a source file, put the imports first, then the constants, then the exported items, then the private functions.
- Use a class when the value holds state. A service with one shared state can be a class that has only static methods. Use a function when the operation holds no state.

## Modules

- Use a named export. Do not use a default export.
- Each folder has an `index.ts` or an `index.tsx` that re-exports the public items of that folder. `framework_fe/src/index.ts` re-exports each feature.
- Use a relative import inside `framework_fe/src/`. Import a type with the `type` keyword.

## Errors

- A type that matches a JSON body from the Rust library uses the same field names as that body.
- A path parameter check throws `Error` before the request.
- An HTTP failure throws `ApiError` or `NetworkConnectionException` from `framework_fe/src/http/network.ts`. Do not return a result object for that failure.

## Tests

- Put the tests in `framework_fe/test/`. Do not put tests in `framework_fe/src/`.
- The folder `framework_fe/test/` has the same shape as `framework_fe/src/`. The test of `framework_fe/src/a/b.ts` is `framework_fe/test/a/b.test.ts`. The test of `framework_fe/src/a/index.tsx` is `framework_fe/test/a/index.test.tsx`.

## Styles

- Each `index.css` file has only one root selector.
- Nest each child selector inside the block of its parent.
- Give the outermost tag of a component a class name with the `g-` prefix. Use the `g-` prefix also for the modifier classes on that tag.
