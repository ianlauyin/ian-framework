# Masking

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Masking

`mask_patterns` is a list of regex patterns. The app gives it to `Logger::new`.

- The syntax is the `regex` crate syntax.
- For a case-insensitive match, start the pattern with `(?i)`.
- The default patterns are `(?i)password`, `(?i)passwd`, `(?i)secret`, `(?i)token`, `(?i)authorization`, `(?i)api[_-]?key` and `(?i)cookie`.
- The middleware logs the request URL with its query. The default patterns keep credential fields out of a log.
- A project adds its own patterns to the list.
- A project that sets a new list drops the default patterns.
- An empty `mask_patterns` list does not change the event.

Scope:

- `Logger` masks the `context` values and the `extra` values. It does not mask the other event fields.
- `Logger::record` masks one event before it stores that event.
- `Logger::direct_send` masks each event before it calls `LogShipper::ship`.
- `HttpLogShipper` does not mask.
- The mask applies to client events from `POST /log` and to request events from the middleware.
- The frontend logger also masks before it sends. The browser patterns and the shipper patterns are different lists. The frontend logger is in `spec/framework_fe/logger.md`.

Key rule:

- When a `context` key or an `extra` key matches a pattern, the logger replaces the full value with the string `***`.
- This rule applies to a value of each type.
- This rule also applies to a key in a nested object of `extra`.
- The logger does not change the key.

Value rule:

- When the key does not match, the logger first masks each `name=value` pair in a string value. The name of the pair has a match of a pattern. The logger replaces the full pair with `***`. The value of the pair ends at `&`, `;` or whitespace.
- Then the logger replaces each match of each pattern in the string value with `***`.
- The patterns apply in list order.
- The value rule also applies to a string in a nested array or a nested object of `extra`.
- The value rule does not mask a number, a `bool`, or null.
