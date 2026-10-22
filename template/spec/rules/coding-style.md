# Coding style

## Duplication and readability

- Readability is more important than strict DRY.
- If simple logic occurs in only a small number of locations, keep it as it is.
- Do not move duplicated logic into a shared helper, a generic type, or a new abstraction only to remove the duplication.

## Comments

- Use the minimum number of comments.
- Make the code clear without comments.
- Write a comment only when the code cannot show the reason.

## Backend (Rust + Axum)

- Use Json for Request and Response unless the route needs special handling.

### Order in a source file

- Put the public types at the top of the file, after the imports and the constants.
- Put the `impl` block of a public type immediately after that type.
- Put the public methods at the top of the `impl` block.

## Source files

- A `pub struct` can have many `pub` methods.
- If a `pub struct` has many `pub` methods, you can make a folder for the struct. Use the struct name in snake case as the folder name. Example: `log_shipper/` for `LogShipper`.
- Keep the tests for the `pub struct`, the `pub fn` or the `pub` method in the same source file, below that item.
- If only one source file uses a helper function, keep the helper in that source file.
- If more than one source file in a module uses a helper function, put the helper in the `mod.rs` file of that module.
- Use `mod.rs` to declare the child modules.
- Use `mod.rs` to declare the items that other modules use.
- Put only declarations and shared helper functions in `mod.rs`.

## Frontend (SolidJS + Typescript)

### Project Structure

- Put UI primitives and controls that have no domain logic in `src/component/**`.
- A module is one feature. It has its own state, its own actions and its own entry component.
- Put each module in `src/module/{module}`. Example: `src/module/login`.
- An area folder is optional. You can put the modules of the same business area in one folder: `src/module/{area}/{module}`. Example: `src/module/wallet/deposit`.

### Contexts

- The global context is in `src/context.tsx`.
- Each module has one context. All the components of the module use this context.
- Write the context of a module in one file: `context.ts` in the module folder.
- Put the actions that are not UI logic in the context of the module.

### Tests

- Put all tests in a `test/` folder next to `src/`. Do not put tests in `src/`.
- In `test/`, use the same folder paths as in `src/`.

### Components

- Give each page an id selector.
- Give the outermost tag of each module component and sub-component a class name. Use this format: `{module}-{component}-{sub-component}...`.
- If only one component uses a sub-component, put the sub-component in the folder of that component.
- Put constants before the component. For example: `const MIN_TRANSFER_AMOUNT = 0.01;`.
- If the component has no styles, use the file `Xxx.tsx`.
- If the component has styles, use the files `Xxx/index.tsx` and `Xxx/index.css`.
- Put the images for the component in an `asset/` folder next to the component.

### Styles

- Put the styles of the component in an `index.css` file next to the component.
- Each `index.css` file has only one root selector.
- Nest each child selector inside the block of its parent.
- To examine nested selectors, run `pnpm lint:css`.

### Class Names

- Give the **outermost tag** of each component in `src/component` a class name with the `g-` prefix.
- Also use the `g-` prefix for modifier classes on that outermost tag.
- If you override the internal parts of a different component, put the `g-` class of that component in the selector.
