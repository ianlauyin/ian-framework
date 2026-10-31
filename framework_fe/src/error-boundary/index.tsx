import { ErrorBoundary, type JSX } from "solid-js";

import { Logger } from "../logger";
import "./index.css";

export interface AppErrorBoundaryProps {
  title: string;
  message: string;
  retryLabel: string;
  reloadLabel: string;
  children: JSX.Element;
}

export const AppErrorBoundary = (props: AppErrorBoundaryProps) => {
  return (
    <ErrorBoundary
      fallback={(error, reset) => {
        logError(error);
        return <ErrorFallback {...props} onReset={reset} />;
      }}
    >
      {props.children}
    </ErrorBoundary>
  );
};

type FallbackProps = Omit<AppErrorBoundaryProps, "children"> & {
  onReset: () => void;
};

function ErrorFallback(props: FallbackProps) {
  return (
    <div id="app-error-boundary" class="g-app-error-boundary">
      <h1>{props.title}</h1>
      <p>{props.message}</p>
      <div class="actions">
        <button type="button" onClick={() => props.onReset()}>
          {props.retryLabel}
        </button>
        <button type="button" onClick={() => window.location.reload()}>
          {props.reloadLabel}
        </button>
      </div>
    </div>
  );
}

// The log route drops an extra value that contains a line break.
function logError(error: unknown) {
  const oneLine = (text: string) => text.replace(/\r?\n\s*/g, " | ");
  if (error instanceof Error) {
    Logger.error("ERROR-BOUNDARY/caught an error", {
      error_name: error.name,
      error_message: oneLine(error.message),
      error_stack: oneLine(error.stack ?? ""),
    });
    return;
  }
  Logger.error("ERROR-BOUNDARY/caught an error", {
    error_message: oneLine(String(error)),
  });
}
