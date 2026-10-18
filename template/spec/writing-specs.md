# Writing specs

For the location of a spec and its two files, read `spec/00_structure.md`.

## Add a spec

- Make a folder with the name `NN_kebab-name/`. `NN` is the next number in the sequence of that package folder.
- Write `business-logic.md` first.
- If a fact can change when the code is refactored, put that fact in `code-impl.md`.
- Add a row to the `## Specs` section of `spec/<package>/00_context.md` of that package.
- If the package has no specs, make the `## Specs` section when you add the first spec.

## Write a spec

Write all spec text in ASD-STE100 (Simplified Technical English).

- If you give several items or several points, use a list.
- If you give only one point, use a paragraph.
- If all items have the same small set of fields, use a table.
- Write in English. Use Traditional Chinese only when you quote a user-facing string exactly.

## Refer to spec/shared/

A rule in `spec/shared/` applies to all apps.

- Before you write a rule, make sure that `spec/shared/` does not already contain it.
- If `spec/shared/` contains the rule, write the path to that rule. Do not write the rule again.
- When a spec refers to a different markdown file, write the full path from the repo root. For example: `spec/shared/00_context.md`.
- Do not write only the filename.
- Do not add a `[label](path)` link.
