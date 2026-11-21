import { createRouter, type RouteDefinition, type RouteSectionProps, useLocation } from "@solidjs/router";
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
    common?: readonly RouteDefinition[];
    unknown: () => void;
    children?: (props: RouteSectionProps) => JSX.Element;
}

export function SwitchRouter<Key extends string>(props: SwitchRouterProps<Key>): JSX.Element {
    const groups = props.keyed.map((group) => ({
        key: group.key,
        Router: createRouter({
            routes: [...routeList(props.common, group.routes), { path: "*", component: () => <Mismatch onMismatch={group.onMismatch} /> }],
        }),
    }));

    return (
        <Switch fallback={<Unknown active={props.active} unknown={props.unknown} />}>
            {groups.map((group) => {
                const Router = group.Router;
                return (
                    <Match when={props.active === group.key}>
                        <Router>{(route) => section(props.children, route)}</Router>
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

function section(children: ((props: RouteSectionProps) => JSX.Element) | undefined, route: RouteSectionProps): JSX.Element {
    if (!children) return route.children;
    return children(route);
}

function routeList(common: readonly RouteDefinition[] | undefined, routes: readonly RouteDefinition[]): RouteDefinition[] {
    if (!common || common.length === 0) return [...routes];
    return wrap(common, 0, [...routes]);
}

function wrap(common: readonly RouteDefinition[], index: number, routes: RouteDefinition[]): RouteDefinition[] {
    if (index >= common.length) return routes;
    return [{ ...common[index], children: wrap(common, index + 1, routes) }];
}
