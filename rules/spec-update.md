# Spec update

When you change the code, update the spec for that code in the same change. A spec that is not up to date is a bug.

The Rust feature index is `spec/framework_rs.md`. The frontend feature index is `spec/framework_fe.md`.

Update the feature document when the change affects the behaviour, a rule, or the contract of that feature.

If the code does not obey a rule in the feature document, and the task did not tell you to change that rule, stop and ask. The code can be incorrect.

Open items follow `spec/framework_rs.md`.

Do not update a spec only for these changes:

- Formatting
- Lint fixes
- Comments
- Dependency version updates
