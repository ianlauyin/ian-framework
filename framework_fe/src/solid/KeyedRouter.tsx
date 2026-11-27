import {createRouter, type RouteDefinition, type RouteSectionProps, useLocation, useNavigate} from "@solidjs/router";
import type {JSX} from "@solidjs/web";
import {createEffect, createMemo, Show} from "solid-js";
import {AsyncContent} from "./async";

export interface KeyedRoutes<Key extends string> {
    key: Key;
    routes: readonly RouteDefinition[];
}

export interface KeyedRouterProps<Key extends string> {
    activeKeys: readonly Key[];
    keyed: readonly KeyedRoutes<Key>[];
    fallbackPath: string;
    loadingOverlay: JSX.Element;
}

export function KeyedRouter<Key extends string>(props: KeyedRouterProps<Key>): JSX.Element {
    const activeGroups = createMemo(() => props.keyed.filter(group => props.activeKeys.includes(group.key)), {equals: sameKeys});

    return (
        <Show when={activeGroups()} keyed>
            {groups => {
                const Layout = (layoutProps: RouteSectionProps) => <AsyncContent loadingOverlay={props.loadingOverlay}>{layoutProps.children}</AsyncContent>;
                const Router = createRouter({
                    routes: [
                        {
                            path: "/",
                            component: Layout,
                            children: [...groups.flatMap(group => group.routes), {path: "*", component: () => <Fallback path={props.fallbackPath} />}],
                        },
                    ],
                });
                return <Router />;
            }}
        </Show>
    );
}

function sameKeys<Key extends string>(previous: readonly KeyedRoutes<Key>[], next: readonly KeyedRoutes<Key>[]): boolean {
    return previous.length === next.length && previous.every((group, index) => group.key === next[index].key);
}

function Fallback(props: {path: string}): JSX.Element {
    const location = useLocation();
    const navigate = useNavigate();
    createEffect(
        () => [location.pathname, props.path] as const,
        ([pathname, path]) => {
            if (pathname === path) return;
            navigate(path, {replace: true});
        }
    );
    return null;
}
