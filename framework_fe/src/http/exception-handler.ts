import { ApiError, NetworkConnectionException } from "./network";

export type ExceptionHandler = (exception: ApiError | NetworkConnectionException) => void | Promise<void>;

let initialized = false;
let handlerRunning = false;

// Gives each ApiError and NetworkConnectionException that no code caught to one handler.
// The handler does not see other errors: Logger.init records them.
export function setupExceptionHandler(handler: ExceptionHandler): void {
  if (initialized) throw new Error("setupExceptionHandler() is called twice");
  initialized = true;

  const handle = async (error: unknown) => {
    if (handlerRunning) return;
    if (!(error instanceof ApiError) && !(error instanceof NetworkConnectionException)) return;

    handlerRunning = true;
    try {
      await handler(error);
    } finally {
      handlerRunning = false;
    }
  };

  window.addEventListener("error", (event) => void handle(event.error));
  window.addEventListener("unhandledrejection", (event) => void handle(event.reason));
}
