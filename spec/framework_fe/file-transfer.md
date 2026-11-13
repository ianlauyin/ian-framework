# File upload and file download

This document describes the file helpers in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The source files are `framework_fe/src/http/upload.ts` and `framework_fe/src/http/download-file.ts`.
- These helpers have no test, because each only passes its arguments to `rawSend` or to the browser. The send step is tested through `spec/framework_fe/ajax.md`.

## API

| Item | Meaning |
| --- | --- |
| `upload<Response>(url, formField, file)` | Sends `file` as `multipart/form-data` with `POST`. `formField` is the field name. The result is the parsed JSON body. |
| `downloadFile(url, name)` | Fetches `url` and saves the body as a file with the name `name`. |

## Behaviour

- Both functions use `url` as it is, with no `/ajax` prefix.
- `upload` uses the send step of `ajax`. The unauthorized handler, the error classes and the date rule are the same as in `spec/framework_fe/ajax.md`. The request has the header `Accept: application/json` and no `Content-Type`, so the browser adds the boundary.
- `downloadFile` uses `credentials: "same-origin"`.
- `downloadFile` throws a `NetworkConnectionException` when the fetch fails or the status is not 2xx. A body of an error response is not saved.
- `downloadFile` makes a temporary link element, clicks it, and removes it.
