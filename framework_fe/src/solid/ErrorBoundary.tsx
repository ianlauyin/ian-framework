import type { JSX } from "@solidjs/web";
import { Errored } from "solid-js";

import { Logger } from "../logger";

export interface ErrorBoundaryProps {
    children: JSX.Element;
    fallback: (reset: () => void) => JSX.Element;
}

export function ErrorBoundary(props: ErrorBoundaryProps): JSX.Element {
    return (
        <Errored
            fallback={(error, reset) => {
                const caught = error();
                const logged = caught instanceof Error ? caught : new Error(String(caught));
                Logger.error("ERROR-BOUNDARY/caughtError", logged);
                return props.fallback(reset);
            }}
        >
            {props.children}
        </Errored>
    );
}
