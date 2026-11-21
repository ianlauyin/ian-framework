import { createRouter, type RouteDefinition, useLocation } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { createEffect, Match, Switch } from "solid-js";

export interface KeyedRoutes<Key extends string> {
    key: Key;
    routes: readonly RouteDefinition[];
    onMismatch: () => void;
}

export interface SwitchRouterProps<Key extends string> {
    active: Key;
    keyed: readonly KeyedRoutes<Key>[];
    shared?: readonly RouteDefinition[];
    unknown: () => void;
}

export function SwitchRouter<Key extends string>(props: SwitchRouterProps<Key>): JSX.Element {
    const groups = props.keyed.map((group) => ({
        key: group.key,
        Router: createRouter({
            routes: [...routeList(props.shared, group.routes), { path: "*", component: () => <Mismatch onMismatch={group.onMismatch} /> }],
        }),
    }));

    return (
        <Switch fallback={<Unknown active={props.active} unknown={props.unknown} />}>
            {groups.map((group) => {
                const Router = group.Router;
                return (
                    <Match when={props.active === group.key}>
                        <Router />
                    </Match>
                );
            })}
        </Switch>
    );
}

function Mismatch(props: { onMismatch: () => void }): JSX.Element {
    const location = useLocation();
    createEffect(
        () => location.pathname,
        () => {
            props.onMismatch();
        },
    );
    return null;
}

function Unknown(props: { active: string; unknown: () => void }): JSX.Element {
    createEffect(
        () => props.active,
        () => {
            props.unknown();
        },
    );
    return null;
}

function routeList(shared: readonly RouteDefinition[] | undefined, routes: readonly RouteDefinition[]): RouteDefinition[] {
    if (!shared || shared.length === 0) return [...routes];
    return wrap(shared, 0, [...routes]);
}

function wrap(shared: readonly RouteDefinition[], index: number, routes: RouteDefinition[]): RouteDefinition[] {
    if (index >= shared.length) return routes;
    return [{ ...shared[index], children: wrap(shared, index + 1, routes) }];
}
