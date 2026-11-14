# Coding style

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Write a comment only when the code cannot show the reason. Write the comment as one complete sentence.
- Return early when a check fails. Keep the success path at the end of the function.
- Write tests only for functions that hold important logic. For each of these functions, test each edge case. Do not test a trivial function, a getter or a pass-through. This rule applies to the Rust code and to the frontend code.
- Name each test by the behavior that it checks.

## Backend (Rust)

The Rust library is the crate `ian_framework` in `framework_rs/`.

- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON: a file upload, a stream or a webhook that has its own format.
- Keep the Rust field name as the JSON name. Add a serde rename only when the contract needs a different name.
- Run `cargo fmt` and `cargo clippy` in `framework_rs/` after you change the code. Fix all the warnings that they show.
- When a cargo feature only adds a step to a function, write one function. Put the step in a `#[cfg(feature = "...")]` block, with an early return when the step gives the result. Do not write a second copy of the function for `#[cfg(not(feature = "..."))]`.

### Configuration

- The library never reads an env variable by a name of its own for an app setting. The project reads its env variables, chooses the names and gives the values to the library. A helper that reads a variable by a name that the caller gives is allowed. That helper is `framework_rs/src/env.rs`.
- The `gcp` module can read the Google Cloud standard env variables `GOOGLE_APPLICATION_CREDENTIALS` and `GOOGLE_CLOUD_PROJECT`. These variables are part of the Application Default Credentials rules, not app settings.

### Modules

- Declare a child module as `pub(crate)` when the parent re-exports the items that a caller uses.
- Re-export those items from the parent module file. A caller imports from the parent module.

### Files

- In a source file, put the imports and the constants first. Then put each public item. For a public type, put its inherent `impl` block with the public methods first, then its trait impls (`Debug`, `Display`). Put the private types, the private functions and their impl blocks after the public items.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import, neither in an import list nor at the start of an import path. To use a module and an item of that module, import only the module. Then use the module name to refer to the item.
- Keep a type, its inherent `impl` block and its trait impls in one file. Do not write an `impl` block for a type in a different file.
- When the sibling files of a module folder use the main type of that folder, put the type in the `mod.rs` of the folder. The sibling files import it from the parent module.

### Errors

- A function that can fail returns `Result<T, Exception>`.
- Build each `Exception` with the `exception!` macro in `framework_rs/src/exception.rs`.
- Put each error code in a `pub const` of type `&str` in the file that owns the failure. The constant name and the string value are the same.
- Write the message as a short sentence. Do not put a secret, a token, a query string or an env value in the message.
- When another error is the cause, pass that error as `source`.

### Tests

- In the tests, do not use a name from a project of the user. This includes repo names, app ids, env variable names, secret names, GCP project ids, service accounts and bucket names. Use generic names: `my-app`, `my-project` and `APP_KEY`.
- Put each test in `framework_rs/src/test/`. Do not put a test in the source file of the tested item. The `#[cfg(test)] mod test;` line in `framework_rs/src/lib.rs` is the only test item in the production code.
- The folder `framework_rs/src/test/` has the same shape as the crate. The tests of `framework_rs/src/a/b.rs` are in `framework_rs/src/test/a/b.rs`. The tests of `framework_rs/src/a/mod.rs` are in `framework_rs/src/test/a/mod.rs`. Each test folder has a `mod.rs` that declares its child modules, with the same `#[cfg(feature = "...")]` attributes as the source.
- A test reaches the tested item with a `crate::` path. When a test needs a private item, change the visibility of the item to `pub(crate)`. Do not use `pub` for this.
- Keep all test-only code in a test module. This includes test constructors, test-only methods and test doubles. Do not add a `#[cfg(test)]` item to the production code.
- Name each `#[test]` function with the behavior that it checks.

### Safety

- Do not show a secret, a token or a query string in `Debug` output or in an error message. For a type that holds a secret, write its `Debug` impl by hand.
- The framework does not panic. Do not use `unwrap`, `expect`, `panic!`, `assert!`, `unreachable!`, `todo!`, `unimplemented!` or an index that can be out of range outside tests. This includes startup code.
- When an input is invalid or an operation fails, return an `Exception`. The project decides how to handle it.

## Frontend (SolidJS + TypeScript)

The frontend library is `framework_fe/`. It is not an application. It has no `src/module/` tree and no application context.

- After you change the frontend code, run `pnpm typecheck` and `pnpm lint` in `framework_fe/`. Fix all the errors that they show. The Biome formatter is off.

### Files

- Put each library file in `framework_fe/src/`. A component with styles uses `framework_fe/src/{name}/index.tsx` and `framework_fe/src/{name}/index.css`.
- In a source file, put the imports first, then the constants, then the exported items, then the private functions.
- Use a class when the value holds state. A service with one shared state can be a class that has only static methods. Use a function when the operation holds no state.

### Modules

- Use a named export. Do not use a default export.
- Each folder has an `index.ts` or an `index.tsx` that re-exports the public items of that folder. `framework_fe/src/index.ts` re-exports each feature.
- Use a relative import inside `framework_fe/src/`. Import a type with the `type` keyword.

### Errors

- A type that matches a JSON body from the Rust library uses the same field names as that body.
- An HTTP failure throws `ApiError` or `NetworkConnectionException` from `framework_fe/src/http/network.ts`. Do not return a result object for that failure.

### Tests

- Put the tests in `framework_fe/test/`. Do not put tests in `framework_fe/src/`.
- The folder `framework_fe/test/` has the same shape as `framework_fe/src/`. The test of `framework_fe/src/a/b.ts` is `framework_fe/test/a/b.test.ts`. The test of `framework_fe/src/a/index.tsx` is `framework_fe/test/a/index.test.tsx`.

### Styles

- Each `index.css` file has only one root selector.
- Nest each child selector inside the block of its parent.
- Give the outermost tag of a component a class name with the `g-` prefix. Use the `g-` prefix also for the modifier classes on that tag.
