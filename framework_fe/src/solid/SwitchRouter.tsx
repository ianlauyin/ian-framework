import { createRouter, type RouteDefinition, type RouteSectionProps, useNavigate } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { Show } from "solid-js";

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
        <Show when={props.isProtected} fallback={<PublicRouter>{(route) => (props.children ? props.children(route) : route.children)}</PublicRouter>}>
            <ProtectedRouter>{(route) => (props.children ? props.children(route) : route.children)}</ProtectedRouter>
        </Show>
    );
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
