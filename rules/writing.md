# Writing markdown

## Write a markdown file

Write all text in ASD-STE100 (Simplified Technical English).

- If you give several items or several points, use a list.
- If you give only one point, use a paragraph.
- If all items have the same small set of fields, use a table.
- Do not write an example. Do not add "for example" text to a rule.

## Refer to a file

- When a markdown file refers to a different file, write the full path from the repo root
- Do not write only the filename.
- Do not add a `[label](path)` link.
- Do not write a home path or an absolute path of the user's computer. To refer to a file in a different repo, write the repo name and the path from the root of that repo.

## Sensitive data

- Do not write sensitive data in a doc. Sensitive data is a cloud project id and project number, a service account email, an external IP address, a bucket name, a registry path that contains the project id, a key, a token and a secret value.
- Write an uppercase placeholder in place of each value. Define each placeholder one time, in a table at the top of the doc, with the location of the real value.
- In a command, write the placeholder as a shell variable, for example `"$PROJECT_ID"`.
