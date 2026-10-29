# Rust framework features

This file is the index of the feature documents for the Rust library in `framework_rs/`.

Each feature has one folder. The `context.md` file of each folder gives the parts that apply to all of the feature and the list of the sub-feature documents.

| Feature | Document |
| --- | --- |
| Request and event logging, and the browser log route `POST /log` | `spec/framework_rs/logging/context.md` |
| Google Cloud Storage client | `spec/framework_rs/gcs/context.md` |
| HTTP boundary error, request header helpers, same-origin middleware, and process version route | `spec/framework_rs/http/context.md` |
| Cookie sessions and password hashes | `spec/framework_rs/auth/context.md` |
| Shutdown signal `ShutdownReceiver` | `spec/framework_rs/shutdown/context.md` |
| Environment variable read helpers. With the `gcp` feature, `required` and `optional` read Secret Manager first. | `spec/framework_rs/env/context.md` |
| Shared Google Cloud code (Cargo feature `gcp`): credentials, access tokens, HTTP client settings and Secret Manager access | `spec/framework_rs/gcp/context.md` |
| Time and time zone helpers, Hong Kong time `hkt_now` | `spec/framework_rs/time/context.md` |

The log shipper posts each batch to `/log/{app}`. `app` is the routing name.

## Open behaviour

An open item is a known behaviour that is not resolved. Each library that has open items has one open-behaviour file. The Rust file is `spec/framework_rs/open-behaviour.json`. The frontend file is `spec/framework_fe/open-behaviour.json`. A feature does not have its own file for open items. Before you change a behaviour, read the open items of its library.

- To add the file for a library, copy `spec/open-behaviour.json` to `spec/framework_rs/open-behaviour.json` or to `spec/framework_fe/open-behaviour.json`.
- The file has only the JSON data: a list of entries. Do not add an HTML page or other text to the file.
- For the Rust library, write one entry for each feature folder that has open items. Use the order of the feature table in this file.
- For the frontend library, write one entry for each feature document that has open items. Use the order of the feature table in `spec/framework_fe.md`.
- Each entry has `spec`, `title`, `context` and `items`. `spec` is the feature folder name, or the feature document name without `.md`. `context` gives the repo, the commit and the date of the last check, and the path conventions.

Each item has these fields:

| Field | Contents |
| --- | --- |
| `id` | The row number in the feature. Do not change it. Do not use it again. |
| `item` | A short name. |
| `now` | What the code does now, with the evidence as `file:line`. |
| `waitingOn` | The teams that make the decision. |
| `nextStep` | The recorded decision. Change it only when a person makes a new decision. |
| `kind` | `bug`, `decision` or `cleanup`. |
| `scope` | The libraries and features that the item has an effect on. |
| `options` | Three ways to resolve the item. Put the recommended way first. |
| `note` | Optional. A reference to a copy of the item in a different library, or a date. |

- Put an item in the file of the library that has the behaviour.
- If an item occurs in both libraries, put a copy in the file of each library. In `note`, give the location of the other copy.
- A Rust feature document refers to its items as `` `spec/framework_rs/open-behaviour.json` (`feature`) ``.
- A frontend feature document refers to its items as `` `spec/framework_fe/open-behaviour.json` (`feature`) ``.
- To resolve an item, fix the code and the spec in the same change. Then delete the item.
- If a feature entry has no items, delete the entry and its references in the feature documents.
- If the Rust library has no open items, delete `spec/framework_rs/open-behaviour.json`.
- If the frontend library has no open items, delete `spec/framework_fe/open-behaviour.json`.

