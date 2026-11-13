# Log shipper

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Shipper

### LogShipper

`ian_framework::logging::LogShipper` is the trait that ships one list of log events.

- The source file is `framework_rs/src/logging/log_shipper/mod.rs`.
- `LogShipper::ship` takes the events and gives `true` only when the send succeeds.
- `HttpLogShipper` is the HTTP implementation of `LogShipper`.

### HttpLogShipper::new

`HttpLogShipper::new(app, endpoint, post_api_key)` builds one shipper. One shipper has one app. The app is the routing name.

- The source file is `framework_rs/src/logging/log_shipper/http_log_shipper.rs`.
- `HttpLogShipper::new` does not read an environment variable or Secret Manager.
- The app reads `endpoint` and `post_api_key` from its own environment variables or secrets, with the names that the app chooses. Then it gives the values to `HttpLogShipper::new`. The app name is fixed in the code of the app. The app can use `ian_framework::env::optional` for this read. The rules of `optional` are in `spec/framework_rs/env/read.md`.
- `post_api_key` is the POST API key of the log server. It is a secret.
- `app` is `&'static str`. `endpoint` is `String`. `post_api_key` is `&str`.
- The app is not checked. The caller gives a valid app name.
- The ship request time limit is 10 seconds.
- The connect time limit of the ship HTTP client is 10 seconds.
- The ship HTTP client does not follow redirects.
- The mask patterns are not an argument of the shipper. `Logger::new` takes them. The default mask patterns are in `spec/framework_rs/logging/masking.md`.
- A clone of one `HttpLogShipper` shares the endpoint, the POST API key and the HTTP client.

`HttpLogShipper::new` returns `Result<HttpLogShipper, Exception>`. It does not panic. The exception has one of these codes:

- `API_KEY_HEADER_INVALID`: the POST API key is not a valid HTTP header value.
- `HTTP_CLIENT_BUILD_FAILED`: the HTTP client cannot be built.

The message and the source of the exception do not hold the POST API key.

`HttpLogShipper::new` writes one `tracing` info event with the field `app` and the message `log shipping posts to the log server`.

### Endpoint and POST API key

- The shipper uses `endpoint` as given. It does not trim, parse or check it.
- The shipper uses `post_api_key` as given. It does not trim or check it, except that the header value must be valid.
- The `Debug` output of `HttpLogShipper` does not show the POST API key. The output shows `***`.

### Ship

`HttpLogShipper::ship` implements `LogShipper::ship`.

- `ship` does not mask. The `Logger` masks the events before it calls `ship`.
- `ship` does not write `app` into the event.
- `ship` does not use the buffer. The buffer rules are in `spec/framework_rs/logging/logger.md`.

The request has these properties:

- The HTTP method is POST.
- The URL is `{endpoint}/log/{app}`. `{app}` is the shipper app.
- The content type is `application/json`.
- The request has the header `Authorization: Bearer {key}`.
- The request time limit is 10 seconds.

The body is one `ian_framework::logging::LogRequest` as JSON: one object with one key `events`. The value of `events` is an array of event objects.

- `LogRequest` has one public field `events: Vec<LogEvent>`. It derives `Serialize` and `Deserialize`.
- The shipper serializes a `LogRequest`. A log server in Rust reads the request with `axum::Json<LogRequest>`. Thus both sides use one type.

- Each event object has each field in the event table. The event table is in `spec/framework_rs/logging/event.md`.
- Each event object has no `app` key.
- Each event object has no `project` key.

Result:

- A response status from 200 through 299 gives `true`.
- A transport error gives `false`.
- An HTTP error status gives `false`.

Log streams:

- The path app selects the log stream.
- A log server stores its project from the path app.
- A consumer splits logs by the app in the path.

## Adapter

The default shipper posts to `{endpoint}/log/{app}`. A consumer that maps the app to a different path uses an adapter. The adapter is outside this shipper.
