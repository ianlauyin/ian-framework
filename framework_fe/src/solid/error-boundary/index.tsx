import { createComponent, ErrorBoundary as SolidErrorBoundary, type JSX } from "solid-js";

import { Logger } from "../../logger";

export interface ErrorBoundaryProps {
  children: JSX.Element;
  fallback: (reset: () => void) => JSX.Element;
}

export function ErrorBoundary(props: ErrorBoundaryProps): JSX.Element {
  return (
      <SolidErrorBoundary
        fallback={(error,  reset) => {
          logError(error);
          return props.fallback(reset);
        }}
      >
        {props.children}
      </SolidErrorBoundary>
    );
}

// The log route drops an extra value that contains a line break.
function logError(error: unknown) {
  const oneLine = (text: string) => text.replace(/\r?\n\s*/g, " | ");
  const message = oneLine(error instanceof Error ? error.message : String(error));
  if (error instanceof Error) {
    Logger.error("ERROR-BOUNDARY/caughtError", message, {
      error_name: error.name,
      error_stack: oneLine(error.stack ?? ""),
    });
    return;
  }
  Logger.error("ERROR-BOUNDARY/caughtError", message);
}
