# Session store

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Session store

A session joins one random token to one subject.

- A session ends when it is idle for `ttl_secs`.
- A session also ends `max_age_secs` after `create`, even when it is in use.
- The subject is one string.
- The project owns the subject.
- The framework does not read the user table.

### SessionConfig

`SessionConfig` gives the session settings. It is a plain struct with `pub` fields and `Default`.

| Field | Default | Value |
| --- | --- | --- |
| `prefix` | `session:` | The start of each Redis key. |
| `cookie_name` | `session` | The name of the session cookie. |
| `ttl_secs` | 604800 | The idle session life in seconds. |
| `max_age_secs` | 2592000 | The longest session life in seconds, from `create`. |

### Connect

`SessionStore::connect` takes one Redis URL and one `SessionConfig`. It opens one Redis connection manager. Then it sends `PING`.

`SessionStore::connect` returns an error in these conditions:

- The URL is not a Redis URL.
- Redis does not reply `PONG`.
- `ttl_secs` is zero.
- `max_age_secs` is zero.
- `max_age_secs` is more than 400 days (34560000 seconds). Browsers limit a cookie lifetime to 400 days.
- `ttl_secs` is more than `max_age_secs`.
- `cookie_name` is not an RFC 6265 cookie name. A cookie name has one or more visible ASCII characters. It has no separator character `()<>@,;:\"/[]?={}`.

The connection manager stops early when Redis is down. It uses these settings:

| Setting | Value |
| --- | --- |
| Connection timeout | 2 seconds |
| Response timeout | 5 seconds |
| Retries | 2 |
| Maximum delay between retries | 500 milliseconds |

- `SessionStore` is a handle. A clone of the handle costs little.
- All clones use the same connection manager.
- The error messages do not contain the Redis URL.

### Redis data

- The Redis key of a session is `{prefix}{hash}`.
- `hash` is the SHA-256 of the token, in lowercase hex.
- Redis does not hold the token. Thus a copy of Redis does not give tokens that work.
- The value is `{deadline}:{subject}`.
- `deadline` is the creation time plus `max_age_secs`, in Unix seconds. The time comes from the clock of the process.
- The key expires after `ttl_secs`.
- Each subject has one Redis set with the key `{prefix}subject:{hash}`. `hash` is the SHA-256 of the subject, in lowercase hex.
- The set holds the Redis keys of the sessions of that subject.
- Each command that writes two or more keys runs as one Lua script. Thus no other command comes between its steps, and it uses one round trip.
- The scripts write keys that are not in the script `KEYS`. Thus the store does not support Redis Cluster.

### Session

`Session` is the result of `create` and `load`.

| Field | Visibility | Value |
| --- | --- | --- |
| `token` | `pub` | The session token. |
| `subject` | `pub` | The subject of the session. |
| `expires_in_secs` | crate only | The seconds to the end of the session at the time of the call. |

- A project cannot make a `Session`. Only `create`, `load` and `slide_session` make one.
- The `Debug` output of `Session` does not show the token.
- `Session` is an Axum extractor. Its use is in `spec/framework_rs/auth/sliding-session.md`.

### Methods

| Method | Result |
| --- | --- |
| `create(subject)` | Makes a new token, stores the session, and returns the `Session`. `expires_in_secs` is `ttl_secs`. |
| `load(token)` | Returns the `Session` and slides its expiry. Returns no session when the session is missing, expired, or past its deadline. |
| `remove(token)` | Removes one session. |
| `remove_subject(subject)` | Removes each session of that subject. |
| `remove_subject_except(subject, keep_token)` | Removes each session of that subject, but not the session of `keep_token`. |
| `ttl_secs()` | Returns the configured `ttl_secs`. |

A token is 32 random bytes from the OS random source, in lowercase hex. A token has 64 characters.

`create` uses these rules:

- It stores the session, adds its key to the set, and sets the expiry of the set to `max_age_secs`, in one script.
- The set holds at most 100 sessions. When the new session makes the set larger, the same script first removes the keys of expired sessions from the set.
- If the set still has more than 100 keys, the script ends the live sessions with the earliest deadline until 100 remain. It never ends the new session.

`load` uses these rules:

- It reads the session and sets the new expiry in one script.
- The new expiry is the smaller of `ttl_secs` and the time to the deadline.
- `expires_in_secs` of the result is the new expiry.
- A value that is not `{deadline}:{subject}` gives no session.
- The subject can contain `:`. The deadline ends at the first `:`.

`remove` uses these rules:

- It deletes the session key with one `DEL`.
- It does not take the key out of the set. The next `create` over the limit or the next `remove_subject` takes it out.
- A session that expires on its own also stays in the set until then, or until the set expires.

`remove_subject` uses these rules:

- It reads the set of the subject.
- It deletes each session key in the set and takes the keys out of the set in one script.
- It does not scan the keys of Redis. Its cost depends on the sessions of the subject only.
- A project does not need a prefix that only the session store uses.
- A session that was created before the set existed is not in a set. `remove_subject` does not end it. It ends when it expires.

`remove_subject_except` uses these rules:

- It does the same as `remove_subject`. It skips the key of `keep_token`, and that key stays in the set.
- `keep_token` continues to work. It does not make a new token.
- A project calls it after a password change. This ends the other sessions of that user and keeps the current session.

## Cookie

`SessionStore::token` reads the session token from the `Cookie` request headers with `ian_framework::http::headers::read_cookie`.

- It takes the value of the first cookie with the name `cookie_name`.
- It returns no token when no cookie has the name.
- It returns no token when the value is not 64 lowercase hex characters. Thus a bad value does not go to Redis.

`SessionStore::cookie` takes one `Session`. It returns one `Set-Cookie` value for the token of the session. The value has these attributes:

- `HttpOnly`
- `Secure`
- `SameSite=Lax`
- `Path=/`
- `Max-Age={expires_in_secs}` of the session. Thus the cookie does not stay after the deadline of the session. The value is at most 400 days.
- `Expires` at the current UTC time plus the same seconds, in the HTTP date format

`SessionStore::clear_cookie` returns one `Set-Cookie` value that removes the cookie.

- It has an empty value.
- It has `Max-Age=0`.
- It has `Expires=Thu, 01 Jan 1970 00:00:00 GMT`.
- Its other attributes are the same as the attributes of `SessionStore::cookie`.
