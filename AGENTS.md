# Framework

This repository is the framework. A project contains the business logic.
Do not put business logic in this repository.

A project copies the files in `template/`.

The Rust library is `crates/ian_framework`.
The default Cargo features do not include `gcs`.

framework structure: `spec/structure.md`

## Things to do

Read `spec/rules/coding-style.md` before the first edit.
Read `spec/rules/git-convention.md` before make git commit.
Read `spec/rules/spec-update.md` after making code changes.
Do not change the files in `template/` unless the user tells you to.
