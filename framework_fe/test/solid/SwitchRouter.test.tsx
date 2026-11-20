import { render } from "@solidjs/web";
import { createSignal, flush } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";

import { SwitchRouter, type KeyedRoutes } from "../../src/solid/SwitchRouter";

enum Area {
    Protected = "protected",
    Public = "public",
    Admin = "admin",
}

let dispose: (() => void) | undefined;

function ProtectedHome() {
    return <p>protected home</p>;
}

function PublicHome() {
    return <p>public home</p>;
}

function AdminHome() {
    return <p>admin home</p>;
}

function groups(): KeyedRoutes<Area>[] {
    return [
        { key: Area.Protected, routes: [{ path: "/", component: ProtectedHome }] },
        { key: Area.Public, routes: [{ path: "/", component: PublicHome }] },
        { key: Area.Admin, routes: [{ path: "/", component: AdminHome }] },
    ];
}

afterEach(() => {
    dispose?.();
    dispose = undefined;
});

describe("SwitchRouter", () => {
    it("shows the routes for the active key", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Protected} routes={groups()} unknown={() => "/missing"} />, root);
        expect(root.textContent).toContain("protected home");
    });

    it("shows another key from the same route list", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Admin} routes={groups()} unknown={() => "/missing"} />, root);
        expect(root.textContent).toContain("admin home");
    });

    it("follows a new active key", () => {
        window.history.pushState({}, "", "/");
        const [active, setActive] = createSignal(Area.Public);
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={active()} routes={groups()} unknown={() => "/missing"} />, root);
        expect(root.textContent).toContain("public home");
        setActive(Area.Protected);
        flush();
        expect(root.textContent).toContain("protected home");
    });

    it("moves an unknown path to the path from unknown", () => {
        window.history.pushState({}, "", "/no-such-page");
        const seen: Area[] = [];
        const root = document.createElement("div");
        dispose = render(
            () => (
                <SwitchRouter
                    active={Area.Public}
                    routes={[
                        { key: Area.Protected, routes: [{ path: "/home", component: ProtectedHome }] },
                        { key: Area.Public, routes: [{ path: "/home", component: PublicHome }] },
                    ]}
                    unknown={(key) => {
                        seen.push(key);
                        return "/home";
                    }}
                />
            ),
            root,
        );
        expect(seen).toEqual([Area.Public]);
        expect(root.textContent).toContain("public home");
    });
});
