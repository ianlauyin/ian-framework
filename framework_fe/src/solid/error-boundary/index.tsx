import {type JSX, ErrorBoundary as SolidErrorBoundary } from "solid-js";

import { Logger } from "../../logger";

export interface ErrorBoundaryProps {
  children: JSX.Element;
  fallback: (reset: () => void) => JSX.Element;
}

export function ErrorBoundary(props: ErrorBoundaryProps): JSX.Element {
  return (
      <SolidErrorBoundary
      fallback={(error, reset) => {
        Logger.error("ERROR-BOUNDARY/caughtError", error);
          return props.fallback(reset);
        }}
      >
        {props.children}
      </SolidErrorBoundary>
    );
}
