# Framework

This repository is the framework.
A project contains the business logic.
Do not put business logic in this repository.

## Contents

| path                    | contains                                  |
| ----------------------- | ----------------------------------------- |
| `spec/rules/`           | shared rules for agents                   |
| `spec/writing-specs.md` | how to write a document in this framework |
| `spec/00_structure.md`  | this file                                 |
| `template/`             | the files that a project copies           |

`spec/rules/` contains `coding-style.md`, `git-convention.md`, and `spec-update.md`.

A project copies the files in `template/`.
`template/spec/` is the spec structure for that project.

## Business logic

In a project, each feature spec has `business-logic.md` and `code-impl.md`.
Read `template/spec/00_structure.md` for those two files.

Do not add these items in this repository:

- a feature package
- `business-logic.md`
- `code-impl.md`
- `spec/shared/`
- `spec/<package>/00_context.md`

## Procedures

To write a document in this repository, read `spec/writing-specs.md`.
To add a feature spec in a project, read `template/spec/writing-specs.md`.
