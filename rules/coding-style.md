# Coding style

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Write a comment only when the code cannot show the reason.
- Write tests only for functions that hold important logic. For each of these functions, test each edge case. Do not test a trivial function, a getter or a pass-through. This rule applies to the Rust code and to the frontend code.

## Backend (Rust)

The Rust library is the crate `ian_framework` in `framework_rs/`.

- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON: a file upload, a stream or a webhook that has its own format.
- Run `cargo fmt` and `cargo clippy` after you change the code. Fix all the warnings that they show.
- When a cargo feature only adds a step to a function, write one function. Put the step in a `#[cfg(feature = "...")]` block, with an early return when the step gives the result. Do not write a second copy of the function for `#[cfg(not(feature = "..."))]`.

### Configuration

- The library never reads an env variable by a name of its own for an app setting. The project reads its env variables, chooses the names and gives the values to the library. A helper that reads a variable by a name that the caller gives, such as `env::required`, is allowed.

### Files

- In a source file, put the imports and the constants first. Then put each public type, its inherent `impl` block with the public methods first, and its trait impls (`Debug`, `Display`). Put the private types, the private functions and their impl blocks after the public items.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import, neither in an import list nor at the start of an import path. To use a module and an item of that module, import only the module. Then use the module name to refer to the item.
- Keep a type, its inherent `impl` block and its trait impls in one file. Do not write an `impl` block for a type in a different file.
- When the sibling files of a module folder use the main type of that folder, put the type in the `mod.rs` of the folder. The sibling files import it from the parent module.

### Tests

- In the tests, do not use a name from a project of the user. This includes repo names, app ids, env variable names, secret names, GCP project ids, service accounts and bucket names. Use generic names: `my-app`, `my-project` and `APP_KEY`.
- Put each test in `src/test/`. Do not put a test in the source file of the tested item. The `#[cfg(test)] mod test;` line in `src/lib.rs` is the only test item in the production code.
- The folder `src/test/` has the same shape as the crate. The tests of `src/a/b.rs` are in `src/test/a/b.rs`. The tests of `src/a/mod.rs` are in `src/test/a/mod.rs`. Each test folder has a `mod.rs` that declares its child modules, with the same `#[cfg(feature = "...")]` attributes as the source.
- A test reaches the tested item with a `crate::` path. When a test needs a private item, change the visibility of the item to `pub(crate)`. Do not use `pub` for this.
- Keep all test-only code in a test module. This includes test constructors, test-only methods and test doubles. Do not add a `#[cfg(test)]` item to the production code.

### Safety

- Do not show a secret, a token or a query string in `Debug` output or in an error message. For a type that holds a secret, write its `Debug` impl by hand.
- Do not use `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` or an index that can be out of range outside tests. Use `get` or return an error.
- Setup or startup work may use `expect` and `panic!` when the input is invalid, such as a constructor that runs once at startup. Write a message that names the cause. Do not put a secret in the message. Request-time code does not use them.

## Frontend (SolidJS + TypeScript)

The frontend library is `framework_fe/`. It is not an application. It has no `src/module/` tree and no application context.

- Put each library file in `framework_fe/src/`. A component with styles uses `framework_fe/src/{name}/index.tsx` and `framework_fe/src/{name}/index.css`.
- Put the tests in `test/`. Do not put tests in `src/`.
- The folder `test/` has the same shape as `src/`. The test of `src/a/b.ts` is `test/a/b.test.ts`. The test of `src/a/index.tsx` is `test/a/index.test.tsx`.
- Each `index.css` file has only one root selector.
- Nest each child selector inside the block of its parent.
- Give the outermost tag of a component a class name with the `g-` prefix. Use the `g-` prefix also for the modifier classes on that tag.
