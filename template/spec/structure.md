# Spec

A spec records the key design decisions, the behaviour, and the requirements. Implementation details stay in the code. A spec refers to the source code. It does not repeat the source code.

Each spec has one folder. The spec tree has the same structure as the package paths.

- Put a spec where the user sees the behaviour. Do not put it where the code is.
- If a flow occurs in two apps, write one spec for each app. Use the same `NN_kebab-name/` folder name in each app. Each spec gives only the data for its app. Each spec refers to the spec for the other app.
- `spec/shared/` contains rules that have no UI.

Each spec has two files:

| file                | contains                                                                                    | read it when                        |
| ------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------- |
| `business-logic.md` | what the feature does and the rules that it must obey: user-visible behaviour, the contract | you decide if a change is permitted |
| `code-impl.md`      | how the current code does it: Prototype, Layout, Persist, Do not                            | you make the change                 |

- `business-logic.md` stays correct after a rewrite of the feature. `code-impl.md` does not stay correct.
- If more than one package uses a term, define the term one time in `spec/shared/00_context.md`.
- If only one package uses a term, define the term in `spec/<package>/00_context.md` of that package.
- Other specs refer to the file that defines the term.

## Procedures

To add or write a spec, read `spec/writing-specs.md`.

## Specs

Each package folder has a `spec/<package>/00_context.md` file. This file contains the Vocab of the package, then the list of its specs.

Before you make the first edit in a package, read `spec/<package>/00_context.md` of that package. Do this for each package that the change touches.

| package                     | scope                                 |
| --------------------------- | ------------------------------------- |
| `spec/shared/00_context.md` | terms that more than one package uses |
