# Coding style

## Duplication and readability

- Readability is more important than strict DRY.
- If simple logic occurs in only a small number of locations, keep it as it is.
- Do not move duplicated logic into a shared helper, a generic type, or a new abstraction only to remove the duplication.
- public type should put at the top of the files after import and const
- public type impl should follow that type
- public method should put at the top of the impl
