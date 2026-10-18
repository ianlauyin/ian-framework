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
| `Cargo.toml`            | the Rust workspace                        |
| `crates/ian_framework/` | the Rust library crate                    |
| `README.md`             | how to build the Rust library             |
| `rust-toolchain.toml`   | the Rust version for this repository      |

`spec/rules/` contains `coding-style.md`, `git-convention.md`, and `spec-update.md`.

A project copies the files in `template/`.
`template/spec/` is the spec structure for that project.

## Rust library

The package name is `ian_framework`.
The crate path is `crates/ian_framework/`.
Cargo features are optional.
The default features are empty.
The `gcs` feature contains the Google Cloud Storage client.
A project sets the Cargo feature `gcs` to compile that client.
The client does not contain product business logic.
Read `README.md` for the build commands.

## Procedures

To write a document in this repository, read `spec/writing-specs.md`.
To add a feature spec in a project, read `template/spec/writing-specs.md`.
