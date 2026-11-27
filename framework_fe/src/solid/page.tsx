import {useIsRouting} from "@solidjs/router";
import type {JSX} from "@solidjs/web";
import {type Component, lazy, Loading as Suspense} from "solid-js";

export interface PageContentProps {
    children?: JSX.Element;
    fallback?: JSX.Element;
    overlay?: JSX.Element;
}

export function page<T extends Component<any>>(load: () => Promise<T>): T {
    return lazy(() => load().then(component => ({default: component})));
}

export function PageContent(props: PageContentProps): JSX.Element {
    const isRouting = useIsRouting();
    const pending = () => !!props.overlay && isRouting();
    return (
        <Suspense fallback={props.fallback ?? props.overlay}>
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
                        {props.overlay}
                    </div>
                ) : null}
            </div>
        </Suspense>
    );
}
