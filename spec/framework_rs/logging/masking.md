# Masking

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Masking

`mask_patterns` is a list of regex patterns.

- The syntax is the `regex` crate syntax.
- For a case-insensitive match, start the pattern with `(?i)`.
- The default patterns are `(?i)password`, `(?i)secret`, and `(?i)token`.
- The middleware logs the query and the JSON body. The default patterns keep credential fields out of a log.
- A project adds its own patterns to the list.
- A project that sets a new list drops the default patterns.
- An empty `mask_patterns` list does not change the event.

Scope:

- The shipper masks only the `extra` values. It does not mask the other event fields.
- The shipper masks in buffered mode and in trace-only mode.
- The shipper masks client events from `POST /log` and request events from the middleware.
- The frontend logger also masks before it sends. The browser patterns and the shipper patterns are different lists. The frontend logger is in `spec/framework_fe/logger.md`.

Key rule:

- When an `extra` key matches a pattern, the shipper replaces the full value with the string `***`.
- This rule applies to a value of each type.
- This rule also applies to a key in a nested object.
- The shipper does not change the key.

Value rule:

- When the key does not match, the shipper replaces each match of each pattern in a string value with `***`.
- The patterns apply in list order.
- This rule also applies to a string in a nested array or a nested object.
- The value rule does not mask a number, a `bool`, or null.
