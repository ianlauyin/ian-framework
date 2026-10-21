# Coding style

## Duplication and readability

- Readability is more important than strict DRY.
- If simple logic occurs in only a small number of locations, keep it as it is.
- Do not move duplicated logic into a shared helper, a generic type, or a new abstraction only to remove the duplication.
- public type should put at the top of the files after import and const
- public type impl should follow that type
- public method should put at the top of the impl

### Source files

- Put one behaviour in one source file.
- A behaviour is one action that other code can start.
- Keep a private helper in the source file of the behaviour that uses the helper.
- Put a helper in a shared file in the same module only when more than one behaviour uses the helper.
- Put the tests in a different file in the same module.
- Use the entry file of a module to declare the child modules.
- Use the entry file of a module to declare the functions that other modules call.
- Do not put a behaviour in the entry file of a module.
- Do not divide one behaviour to make the file shorter.
