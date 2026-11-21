import { createRouter, type RouteDefinition, type RouteSectionProps } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { Match, Switch } from "solid-js";

export interface KeyedRoutes<Key extends string> {
    key: Key;
    routes: readonly RouteDefinition[];
}

export interface SwitchRouterProps<Key extends string> {
    keyed: readonly KeyedRoutes<Key>[];
    active: Key;
    common?: readonly RouteDefinition[];
    children?: (props: RouteSectionProps) => JSX.Element;
}

export function SwitchRouter<Key extends string>(props: SwitchRouterProps<Key>): JSX.Element {
    const groups = props.keyed.map((group) => ({
        key: group.key,
        Router: createRouter({
            routes: routeList(props.common, group.routes),
        }),
    }));

    return (
        <Switch fallback={null}>
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
