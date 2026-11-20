import { createRouter, type RouteDefinition, type RouteSectionProps, useNavigate } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { Match, Switch } from "solid-js";

export interface KeyedRoutes<Key extends string> {
    key: Key;
    routes: readonly RouteDefinition[];
}

export interface SwitchRouterProps<Key extends string> {
    routes: readonly KeyedRoutes<Key>[];
    active: Key;
    unknown: (key: Key) => string;
    children?: (props: RouteSectionProps) => JSX.Element;
}

export function SwitchRouter<Key extends string>(props: SwitchRouterProps<Key>): JSX.Element {
    const groups = props.routes.map((group) => ({
        key: group.key,
        Router: createRouter({
            routes: [...group.routes, catchAll(() => props.unknown(group.key))],
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

function catchAll(path: () => string): RouteDefinition {
    return {
        path: "*404",
        component: () => <UnknownPage path={path()} />,
    };
}

function UnknownPage(props: { path: string }): JSX.Element {
    const navigate = useNavigate();
    navigate(props.path);
    return null;
}
