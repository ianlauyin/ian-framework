# Coding style

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Write a comment only when the code cannot show the reason.

## Backend (Rust)

The Rust library is the crate `ian_framework` in `framework_rs/`.

- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON: a file upload, a stream or a webhook that has its own format.
- Do not write doc comments. This includes `///` and `//!`.
- Run `cargo fmt` and `cargo clippy` after you change the code. Fix all the warnings that they show.
- When a cargo feature only adds a step to a function, write one function. Put the step in a `#[cfg(feature = "...")]` block, with an early return when the step gives the result. Do not write a second copy of the function for `#[cfg(not(feature = "..."))]`.

### Configuration

- The library never reads an env variable by a name of its own for an app setting. The project reads its env variables, chooses the names and gives the values to the library. A helper that reads a variable by a name that the caller gives, such as `env::required`, is allowed.
- A project reads a value that is not a secret, such as a bucket name, a list, a path, a URL or a log level, only from the process environment. Do not read that value from Secret Manager. When a value is not a secret, do not create a secret for it.

### Files

- In a source file, put the imports and the constants first. Then put each public type, its inherent `impl` block with the public methods first, and its trait impls (`Debug`, `Display`). Put the private types, the private functions and their impl blocks after the public items.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import, neither in an import list nor at the start of an import path. To use a module and an item of that module, import only the module. Then use the module name to refer to the item.
- Keep a helper, a constant or a type in the file that uses it. When a different file also uses it, put it in the public module of its domain. Put a time or time zone helper in the `time` module.

### Tests

- In the tests, do not use a name from a project of the user. This includes repo names, app ids, env variable names, secret names, GCP project ids, service accounts and bucket names. Use generic names: `my-app`, `my-project` and `APP_KEY`.
- Put the tests in one `#[cfg(test)] mod tests` at the end of the source file that holds the tested item.
- Keep each test helper in the test module that uses it. If the tests of two or more files use a test-only item, put the item in the `#[cfg(test)] mod tests` of the parent module. The other test modules can use it.
- Keep all test-only code in a test module. This includes test constructors, test-only methods and test doubles. Do not add a `#[cfg(test)]` item to the production code.

### Safety

- Do not show a secret, a token or a query string in `Debug` output or in an error message. For a type that holds a secret, write its `Debug` impl by hand.
- Do not use `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` or an index that can be out of range outside tests. Use `get` or return an error.

## Frontend (SolidJS + TypeScript)

The frontend library is `framework_fe/`. It is not an application. It has no `src/module/` tree and no application context.

- Put each library file in `framework_fe/src/`. A component with styles uses `framework_fe/src/{name}/index.tsx` and `framework_fe/src/{name}/index.css`.
- Put the tests in `framework_fe/test/`. Do not put tests in `framework_fe/src/`.
- Each `index.css` file has only one root selector.
- Nest each child selector inside the block of its parent.
- Give the outermost tag of a component a class name with the `g-` prefix. Use the `g-` prefix also for the modifier classes on that tag.
