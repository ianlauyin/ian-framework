import {useIsRouting} from "@solidjs/router";
import type {JSX} from "@solidjs/web";
import {type Component, lazy, Loading as Suspense} from "solid-js";

export interface AsyncContentProps {
    children?: JSX.Element;
    loadingOverlay: JSX.Element;
}

export function async<T extends Component<any>>(load: () => Promise<T>): T {
    return lazy(() => load().then(component => ({default: component})));
}

export function AsyncContent(props: AsyncContentProps): JSX.Element {
    const isRouting = useIsRouting();
    const pending = () => isRouting();
    return (
        <Suspense fallback={props.loadingOverlay}>
            <div style={{position: "relative"}} aria-busy={pending() ? "true" : "false"}>
                <div
                    style={{
                        opacity: pending() ? 0.5 : 1,
                        "pointer-events": pending() ? "none" : undefined,
                    }}
                >
                    {props.children}
                </div>
                {pending() ? (
                    <div
                        style={{
                            position: "absolute",
                            inset: "0",
                            display: "flex",
                            "align-items": "center",
                            "justify-content": "center",
                        }}
                    >
                        {props.loadingOverlay}
                    </div>
                ) : null}
            </div>
        </Suspense>
    );
}
