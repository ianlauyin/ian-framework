# Debugging

When you fix a bug or find the cause of an error, give evidence for each claim.

- Evidence is a log line, an error message, a command output, a line of code, a diff, a test result or an official document. Give its source: the command, the file path and line, the build id or the URL.
- Do not give a cause from memory or from a pattern that is usually true. Find the evidence first.
- If you do not have the evidence, say that the cause is not known. Then give the command or the test that gets the evidence. Do not change the code until you have the evidence.
- If two events occur at the same time, they are not necessarily cause and effect. Say that it is only a correlation, until you have evidence for the cause.
- If the evidence does not agree with your claim, remove the claim and tell the user.
