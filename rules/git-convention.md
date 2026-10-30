# Git convention

- To pull, use rebase: `git pull --rebase`.
- To integrate changes, use rebase. Do not make a merge commit.
- Do not force push. Do not use `--force` or `--force-with-lease`.
- After you push a commit, do not change it. Do not squash, amend, or rebase a pushed commit.

## Git commits

- Write the commit message on one line, in the format `scope: subject`.
- Usually, do not add a body. Add a body only if the subject cannot fully describe the change.
