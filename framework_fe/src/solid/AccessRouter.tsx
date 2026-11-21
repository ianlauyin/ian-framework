import { createRouter, type RouteDefinition, useLocation, useNavigate } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { createEffect, createMemo, Show } from "solid-js";

export type AccessRoute = RouteDefinition & {
    canAccess?: () => boolean;
};

export interface AccessRouterProps {
    routes: readonly AccessRoute[];
    fallbackPath: string;
}

export function AccessRouter(props: AccessRouterProps): JSX.Element {
    const access = createMemo(() => props.routes.map((route) => (canAccess(route) ? "1" : "0")).join(""));

    return (
        <Show when={access()} keyed>
            {(access) => {
                const Router = createRouter({
                    routes: [...props.routes.filter((_, index) => access[index] === "1"), { path: "*", component: () => <Fallback path={props.fallbackPath} /> }],
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

function canAccess(route: AccessRoute): boolean {
    if (!route.canAccess) return true;
    return route.canAccess();
}
