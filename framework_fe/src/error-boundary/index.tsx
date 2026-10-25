import { ErrorBoundary, JSX } from "solid-js";
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
      fallback={(_error, reset) => <ErrorFallback {...props} onReset={reset} />}
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
