import { createRouter, type RouteDefinition, type RouteSectionProps, useNavigate } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { Match, Switch } from "solid-js";

export interface SwitchRouterProps {
    protectedRoutes: readonly RouteDefinition[];
    publicRoutes: readonly RouteDefinition[];
    isProtected: boolean;
    unknown: (isProtected: boolean) => string;
    children?: (props: RouteSectionProps) => JSX.Element;
}

export function SwitchRouter(props: SwitchRouterProps): JSX.Element {
    const ProtectedRouter = createRouter({
        routes: [...props.protectedRoutes, catchAll(() => props.unknown(true))],
    });
    const PublicRouter = createRouter({
        routes: [...props.publicRoutes, catchAll(() => props.unknown(false))],
    });

    return (
        <Switch>
            <Match when={props.isProtected}>
                <ProtectedRouter>{(route) => section(props, route)}</ProtectedRouter>
            </Match>
            <Match when={!props.isProtected}>
                <PublicRouter>{(route) => section(props, route)}</PublicRouter>
            </Match>
        </Switch>
    );
}

function section(props: SwitchRouterProps, route: RouteSectionProps): JSX.Element {
    if (!props.children) return route.children;
    return props.children(route);
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
