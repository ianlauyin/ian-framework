# Framework

This repository is the framework. A project contains the business logic.
Do not put business logic in this repository.

The Rust library is `framework_rs/`.
The frontend library is `framework_fe/`.

Rust framework structure: `spec/framework_rs.md`
Frontend framework structure: `spec/framework_fe.md`

Before you change a behaviour, read the open items of its library. The rules are in `spec/framework_rs.md`.

## Rules

| File | When to read |
| --- | --- |
| `rules/coding-style.md` | Before the first edit |
| `rules/debugging.md` | Before you fix a bug or find the cause of an error |
| `rules/git-convention.md` | Before you make a git commit |
| `rules/spec-update.md` | After you change the code |
| `rules/writing.md` | Before you write or update a markdown file |
