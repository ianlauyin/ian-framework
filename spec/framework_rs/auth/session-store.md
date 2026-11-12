# Session store

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Session store

A session joins one random token to one subject.

- A session ends when it is idle for `ttl_secs`.
- A session has no absolute end. A session that is in use continues until `remove`, `remove_subject` or the limit of `max_sessions_per_subject` ends it.
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
| `max_sessions_per_subject` | 100 | The most live sessions of one subject. A subject that logs in again and again does not make the Redis set grow without a bound. |

### Create

`SessionStore::new` takes one `redis::aio::ConnectionManager` and one `SessionConfig`. The project makes the connection manager with `ian_framework::redis::connect`, which also checks the connection. `new` is not async and does not touch Redis.

`SessionStore::new` returns an error in these conditions:

- `ttl_secs` is zero.
- `ttl_secs` is more than 400 days (34560000 seconds). Browsers limit a cookie lifetime to 400 days.
- `max_sessions_per_subject` is zero.
- `cookie_name` is not an RFC 6265 cookie name. A cookie name has one or more visible ASCII characters. It has no separator character `()<>@,;:\"/[]?={}`.

- `SessionStore` is a handle. A clone of the handle costs little.
- All clones use the same connection manager.
### Redis data

- The Redis key of a session is `{prefix}{hash}`.
- `hash` is the SHA-256 of the token, in lowercase hex.
- Redis does not hold the token. Thus a copy of Redis does not give tokens that work.
- The value is the subject.
- The key expires after `ttl_secs`.
- Each subject has one Redis set with the key `{prefix}subject:{hash}`. `hash` is the SHA-256 of the subject, in lowercase hex.
- The set holds the Redis keys of the sessions of that subject.
- The set has no expiry. A live session can continue for any time, so a set expiry could hide a live session from `remove_subject`.
- The store does not use Lua scripts.
- A transaction writes keys in different hash slots. Thus the store does not support Redis Cluster.
- `load` uses `GETEX`. Thus the store needs Redis 6.2 or later.

### Session

`Session` is the result of `create` and `load`.

| Field | Visibility | Value |
| --- | --- | --- |
| `token` | `pub` | The session token. |
| `subject` | `pub` | The subject of the session. |

- The `Debug` output of `Session` does not show the token.
- `Session` is an Axum extractor. Its use is in `spec/framework_rs/auth/session-middleware.md`.

### Methods

| Method | Result |
| --- | --- |
| `create(subject)` | Makes a new token, stores the session, and returns the `Session`. |
| `load(token)` | Returns the `Session` and slides its expiry. Returns no session when the session is missing or expired. |
| `remove(token)` | Removes one session. |
| `remove_subject(subject)` | Removes each session of that subject. |
| `remove_subject_except(subject, keep_token)` | Removes each session of that subject, but not the session of `keep_token`. |
| `ttl_secs()` | Returns the configured `ttl_secs`. |

A token is 32 random bytes from the OS random source, in lowercase hex. A token has 64 characters.

`create` uses these rules:

- It stores the session, adds its key to the set, and reads the size of the set, in one transaction.
- The set holds at most `max_sessions_per_subject` sessions. When the new session makes the set larger, `create` reads the set and the `TTL` of each key in one pipeline. Then it removes the keys of expired sessions from the set.
- If the set still has more than `max_sessions_per_subject` keys, `create` ends the live sessions with the smallest `TTL` until `max_sessions_per_subject` remain. These are the sessions that were used least recently. It never ends the new session. It does these removals in one transaction.
- Two `create` calls for one subject at the same time can both trim the set. Thus the set can have fewer than `max_sessions_per_subject` sessions after them.

`load` uses these rules:

- It reads the session and sets the expiry to `ttl_secs` with one `GETEX`. This is one round trip.

`remove` uses these rules:

- It deletes the session key with one `DEL`.
- It does not take the key out of the set. The next `create` over the limit or the next `remove_subject` takes it out.
- A session that expires on its own also stays in the set until then.

`remove_subject` uses these rules:

- It reads the set of the subject.
- It deletes each session key in the set and takes the keys out of the set in one transaction.
- A session that is created between the read and the transaction stays.
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

`SessionStore::cookie` takes one token. It returns one `Set-Cookie` value for the token. The value has these attributes:

- `HttpOnly`
- `Secure`
- `SameSite=Lax`
- `Path=/`
- `Max-Age={ttl_secs}`

The cookie has no `Expires` attribute. All current browsers use `Max-Age`. A browser that reads only `Expires` keeps the cookie until the browser closes.

`SessionStore::clear_cookie` returns one `Set-Cookie` value that removes the cookie.

- It has an empty value.
- It has `Max-Age=0`.
- Its other attributes are the same as the attributes of `SessionStore::cookie`.
