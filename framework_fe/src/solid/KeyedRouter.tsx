import {createRouter, type RouteDefinition, type RouteSectionProps, useLocation, useNavigate} from "@solidjs/router";
import type {JSX} from "@solidjs/web";
import {type Component, createEffect, createMemo, createSignal, Show, untrack} from "solid-js";

import {AsyncContent} from "./async";

const PRELOAD_COMPONENT = Symbol();
const DEFAULT_LOADING_DELAY = 150;

type RouteComponent = NonNullable<RouteDefinition["component"]>;

export interface KeyedRoutes<Key extends string> {
    key: Key;
    routes: readonly RouteDefinition[];
}

export interface KeyedRouterProps<Key extends string> {
    activeKeys: readonly Key[];
    keyed: readonly KeyedRoutes<Key>[];
    fallbackPath: string;
    loadingOverlay: JSX.Element;
    loadingDelay?: number;
}

export function KeyedRouter<Key extends string>(props: KeyedRouterProps<Key>): JSX.Element {
    const activeGroups = createMemo(() => props.keyed.filter(group => props.activeKeys.includes(group.key)), {equals: sameKeys});
    const initialGroups = untrack(activeGroups);
    const [shown, setShown] = createSignal(initialGroups);
    const overlay = () => props.loadingOverlay;
    const delay = () => props.loadingDelay ?? DEFAULT_LOADING_DELAY;
    let shownGroups = initialGroups;
    let request = 0;

    createEffect(
        () => ({groups: activeGroups(), keyed: props.keyed, fallbackPath: props.fallbackPath}),
        source => {
            const next = source.groups;
            if (sameKeys(shownGroups, next)) return;
            const token = ++request;
            const preloadSource = {activeKeys: next.map(group => group.key), keyed: source.keyed, fallbackPath: source.fallbackPath};
            const path = location.pathname;
            const reveal = () => {
                if (token !== request) return;
                if (location.pathname === path) {
                    const target = pagePath(preloadSource, path);
                    if (target !== path) window.history.replaceState(window.history.state, "", target);
                }
                shownGroups = next;
                setShown(next);
            };
            preloadKeyedRoutes(preloadSource, path).then(reveal, reveal);
            return () => {
                if (request === token) request++;
            };
        }
    );

    return (
        <Show when={shown()} keyed>
            {groups => {
                const Router = createRouter({
                    routes: [
                        ...bindRoutes(
                            groups.flatMap(group => group.routes),
                            overlay,
                            delay
                        ),
                        {path: "*", component: () => <Fallback path={props.fallbackPath} />},
                    ],
                });
                return <Router />;
            }}
        </Show>
    );
}

export function preloadKeyedRoutes<Key extends string>(source: Pick<KeyedRouterProps<Key>, "activeKeys" | "keyed" | "fallbackPath">, path = location.pathname): Promise<void> {
    try {
        const routes = activeRoutes(source);
        const components = matchComponents(routes, path) ?? matchComponents(routes, source.fallbackPath) ?? [];
        return preloadComponents(components);
    } catch (error) {
        return Promise.reject(error);
    }
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

function pagePath<Key extends string>(source: Pick<KeyedRouterProps<Key>, "activeKeys" | "keyed" | "fallbackPath">, path: string): string {
    const routes = activeRoutes(source);
    if (matchComponents(routes, path)) return path;
    return source.fallbackPath;
}

function activeRoutes<Key extends string>(source: Pick<KeyedRouterProps<Key>, "activeKeys" | "keyed">): RouteDefinition[] {
    return source.keyed.filter(group => source.activeKeys.includes(group.key)).flatMap(group => group.routes);
}

function matchComponents(routes: readonly RouteDefinition[], path: string): unknown[] | undefined {
    const matcher = createRouter({routes: tagRoutes(routes)});
    const matches = untrack(() => matcher.match(path));
    if (matches.length === 0) return;
    return matches.map(match => preloadTarget(match.info));
}

function preloadTarget(info: object | undefined): unknown {
    if (!info) return;
    return (info as Record<symbol, unknown>)[PRELOAD_COMPONENT];
}

function tagRoutes(routes: readonly RouteDefinition[]): RouteDefinition[] {
    return routes.map(tagRoute);
}

function tagRoute(route: RouteDefinition): RouteDefinition {
    const info = {...route.info};
    (info as Record<symbol, unknown>)[PRELOAD_COMPONENT] = route.component;
    const children = route.children;
    if (typeof children === "function" || children == null) return {...route, info};
    const list = Array.isArray(children) ? children : [children];
    return {...route, info, children: list.map(tagRoute)};
}

function preloadComponents(components: readonly unknown[]): Promise<void> {
    const pending: Promise<unknown>[] = [];
    for (const component of components) {
        const task = componentPreload(component);
        if (task) pending.push(task);
    }
    return Promise.all(pending).then(() => undefined);
}

function componentPreload(component: unknown): Promise<unknown> | undefined {
    if (typeof component !== "function" || !("preload" in component)) return;
    const preload = component.preload;
    if (typeof preload !== "function") return;
    try {
        const result: unknown = preload();
        if (typeof result === "object" && result !== null && "then" in result && typeof result.then === "function") return result as Promise<unknown>;
    } catch (error) {
        return Promise.reject(error);
    }
}

function bindRoutes(routes: readonly RouteDefinition[], overlay: () => JSX.Element, delay: () => number, covered = false): RouteDefinition[] {
    const bound: RouteDefinition[] = [];
    const leaves: RouteDefinition[] = [];
    const flushLeaves = () => {
        if (leaves.length === 0) return;
        const grouped = leaves.splice(0, leaves.length);
        if (covered) {
            bound.push(...grouped);
            return;
        }
        bound.push({component: (props: RouteSectionProps) => boundary(overlay, delay, props.children), children: grouped});
    };
    for (const route of routes) {
        const nested = childRoutes(route);
        if (typeof route.children === "function") {
            flushLeaves();
            const component = route.component;
            bound.push(component && !covered ? {...route, component: boundLayout(component, overlay, delay)} : route);
            continue;
        }
        if (nested && route.component && !covered) {
            flushLeaves();
            bound.push({
                ...route,
                component: boundLayout(route.component, overlay, delay),
                children: bindRoutes(nested, overlay, delay, true),
            });
            continue;
        }
        if (nested) {
            flushLeaves();
            bound.push({...route, children: bindRoutes(nested, overlay, delay, covered)});
            continue;
        }
        if (route.component && !covered) {
            leaves.push(route);
            continue;
        }
        flushLeaves();
        bound.push(route);
    }
    flushLeaves();
    return bound;
}

function childRoutes(route: RouteDefinition): readonly RouteDefinition[] | undefined {
    const children = route.children;
    if (children == null || typeof children === "function") return;
    const list = Array.isArray(children) ? children : [children];
    if (list.length === 0) return;
    return list;
}

function boundLayout(component: RouteComponent, overlay: () => JSX.Element, delay: () => number): RouteComponent {
    return (props: RouteSectionProps) => {
        const Layout = component as Component<RouteSectionProps>;
        return <Layout {...props}>{boundary(overlay, delay, props.children)}</Layout>;
    };
}

function boundary(overlay: () => JSX.Element, delay: () => number, children: JSX.Element | undefined): JSX.Element {
    return (
        <AsyncContent loadingOverlay={overlay()} loadingDelay={delay()}>
            {children}
        </AsyncContent>
    );
}
