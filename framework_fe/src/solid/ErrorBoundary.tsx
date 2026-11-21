import type {JSX} from "@solidjs/web";
import {Errored, type Component} from "solid-js";

import {Logger} from "../logger";

export interface ErrorBoundaryProps {
    children: JSX.Element;
    fallback: Component<{reset: () => void}>;
}

export function ErrorBoundary(props: ErrorBoundaryProps): JSX.Element {
    return (
        <Errored
            fallback={(error, reset) => {
                const caught = error();
                const logged = caught instanceof Error ? caught : new Error(String(caught));
                Logger.error("ERROR-BOUNDARY/caughtError", logged);
                const Fallback = props.fallback;
                return <Fallback reset={reset} />;
            }}
        >
            {props.children}
        </Errored>
    );
}
