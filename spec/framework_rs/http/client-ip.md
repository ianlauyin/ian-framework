# Client IP middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Client IP middleware

- The source file is `framework_rs/src/http/middleware/set_client_ip.rs`.
- `ian_framework::http::middleware::set_client_ip` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `ClientIpSource`.
- The middleware puts one `ClientIp` in the request extensions, and then calls the next handler. It never rejects a request.

| Type | Meaning |
| --- | --- |
| `ClientIp(pub String)` | The client IP as text. The value is empty when no source gives an IP. |
| `ClientIpSource` | Which sources the middleware trusts. |

`ClientIpSource` has these variants:

| Variant | Sources |
| --- | --- |
| `Headers` | The headers `cf-connecting-ip`, `x-forwarded-for` and `x-real-ip`, in this order. Then the host of `ConnectInfo<SocketAddr>`. The rules are the `ip` rules in `spec/framework_rs/logging/middleware.md`. |
| `Header(name)` | Only the header `name`, with the whitespace removed. When the header is missing or blank, the host of `ConnectInfo<SocketAddr>`. |
| `ForwardedFor { trusted_hops }` | The entry of `x-forwarded-for` that is `trusted_hops` places from the right end. All `x-forwarded-for` fields are joined in order, and each entry is split at `,`. When `trusted_hops` is 0, or the header has no such entry, or the entry is blank, the host of `ConnectInfo<SocketAddr>`. |
| `Socket` | Only the host of `ConnectInfo<SocketAddr>`. The middleware ignores all headers. |

Usage rules:

- Use `Headers` only when a trusted proxy is the only way to reach the process. A client that reaches the process directly can send any value in these headers.
- Use `Header` when the proxy sets or overwrites one header. Use the header name that the proxy sets.
- Use `ForwardedFor` when each trusted proxy adds one entry at the right end of `x-forwarded-for`. Set `trusted_hops` to the number of trusted proxies. A client can only add entries at the left end, so a client cannot choose the selected entry.
- Use `Socket` when clients reach the process directly.
- The server must be started with `into_make_service_with_connect_info::<SocketAddr>()`. Without it, `Socket` gives an empty IP and the middleware logs a warning.
- `log_request` and `rate_limit` read `ClientIp` from the request extensions. When the request has no `ClientIp`, they use the `Headers` rules.
- Put the middleware outside `log_request` and `rate_limit`.
