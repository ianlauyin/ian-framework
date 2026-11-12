# Redis helpers

This document describes the Redis helpers in the framework.
This feature is not product business logic.

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::redis`.
- The source file is `framework_rs/src/redis.rs`.
- This module has no tests.

## Feature `redis`

- The Cargo feature name is `redis`.
- `redis` is not a default feature.
- The `auth` feature enables `redis`.
- The `redis` feature enables the `redis` crate feature `tokio-rustls-comp`. Thus a `rediss://` URL connects with TLS.
- The TLS connection uses `rustls` with the system root certificates.
- A project does not add its own `redis` dependency for TLS.

## Functions

| Function | Behavior |
| --- | --- |
| `connect(url)` | Takes one Redis URL. Opens one `redis::aio::ConnectionManager`. Sends `PING`. Returns the connection manager when Redis replies `PONG`. |

`connect` returns an `Exception` in these conditions:

| Code | Condition |
| --- | --- |
| `REDIS_CONFIG` | The URL is not a Redis URL. |
| `REDIS_UNREACHABLE` | The connection fails. The `PING` command fails. Redis does not reply `PONG`. |

The error messages do not contain the Redis URL.

The connection manager stops early when Redis is down. It uses these settings:

| Setting | Value |
| --- | --- |
| Connection timeout | 2 seconds |
| Response timeout | 5 seconds |
| Retries | 2 |
| Maximum delay between retries | 500 milliseconds |

## Out of scope

This feature does not include these items:

- A Redis command other than `PING`.
