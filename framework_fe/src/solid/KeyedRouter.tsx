import { createRouter, type RouteDefinition, useLocation, useNavigate } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { createEffect, createMemo, Show } from "solid-js";

export interface KeyedRoutes<Key extends string> {
    key: Key;
    routes: readonly RouteDefinition[];
}

export interface KeyedRouterProps<Key extends string> {
    activeKeys: readonly Key[];
    keyed: readonly KeyedRoutes<Key>[];
    fallbackPath: string;
}

export function KeyedRouter<Key extends string>(props: KeyedRouterProps<Key>): JSX.Element {
    const active = createMemo(() => props.keyed.map((group) => (props.activeKeys.includes(group.key) ? "1" : "0")).join(""));

    return (
        <Show when={active()} keyed>
            {(active) => {
                const routes = props.keyed.filter((_, index) => active[index] === "1").flatMap((group) => group.routes);
                const Router = createRouter({
                    routes: [...routes, { path: "*", component: () => <Fallback path={props.fallbackPath} /> }],
                });
                return <Router />;
            }}
        </Show>
    );
}

function Fallback(props: { path: string }): JSX.Element {
    const location = useLocation();
    const navigate = useNavigate();
    createEffect(
        () => [location.pathname, props.path] as const,
        ([pathname, path]) => {
            if (pathname === path) return;
            navigate(path, { replace: true });
        },
    );
    return null;
}
