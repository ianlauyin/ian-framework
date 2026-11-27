import {useIsRouting} from "@solidjs/router";
import type {JSX} from "@solidjs/web";
import {type Component, createEffect, createSignal, lazy, Loading as Suspense, onCleanup} from "solid-js";

const DEFAULT_LOADING_DELAY = 150;
let boundaries = 0;

export interface AsyncContentProps {
    children?: JSX.Element;
    loadingOverlay: JSX.Element;
    loadingDelay?: number;
}

export function async<T extends Component<any>>(load: () => Promise<T>): T {
    return lazy(() => load().then(component => ({default: component})));
}

export function AsyncContent(props: AsyncContentProps): JSX.Element {
    const suspend = boundaries === 0;
    boundaries++;
    onCleanup(() => {
        boundaries--;
    });
    const isRouting = useIsRouting();
    const [delayed, setDelayed] = createSignal(false);
    createEffect(
        () => [isRouting(), props.loadingDelay ?? DEFAULT_LOADING_DELAY] as const,
        ([routing, delay]) => {
            if (!routing) {
                setDelayed(false);
                return;
            }
            const timer = setTimeout(() => setDelayed(true), delay);
            return () => clearTimeout(timer);
        }
    );
    const pending = () => isRouting() && delayed();
    if (!suspend) {
        return (
            <Frame pending={pending()} loadingOverlay={props.loadingOverlay}>
                {props.children}
            </Frame>
        );
    }
    return (
        <Suspense fallback={props.loadingOverlay}>
            <Frame pending={pending()} loadingOverlay={props.loadingOverlay}>
                {props.children}
            </Frame>
        </Suspense>
    );
}

interface FrameProps {
    pending: boolean;
    loadingOverlay: JSX.Element;
    children?: JSX.Element;
}

function Frame(props: FrameProps): JSX.Element {
    return (
        <div style={{position: "relative"}} aria-busy={props.pending ? "true" : "false"}>
            <div
                style={{
                    opacity: props.pending ? 0.5 : 1,
                    "pointer-events": props.pending ? "none" : undefined,
                }}
            >
                {props.children}
            </div>
            {props.pending ? (
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
    );
}
