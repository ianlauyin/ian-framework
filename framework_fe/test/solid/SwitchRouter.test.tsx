import { type RouteSectionProps } from "@solidjs/router";
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

function Shell(props: RouteSectionProps) {
    return (
        <div>
            <p>shell</p>
            {props.children}
        </div>
    );
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
        dispose = render(() => <SwitchRouter active={Area.Protected} keyed={groups()} />, root);
        expect(root.textContent).toContain("protected home");
    });

    it("shows another key from the same route list", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Admin} keyed={groups()} />, root);
        expect(root.textContent).toContain("admin home");
    });

    it("follows a new active key", () => {
        window.history.pushState({}, "", "/");
        const [active, setActive] = createSignal(Area.Public);
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={active()} keyed={groups()} />, root);
        expect(root.textContent).toContain("public home");
        setActive(Area.Protected);
        flush();
        expect(root.textContent).toContain("protected home");
    });

    it("shows the common route around the active routes", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Public} keyed={groups()} common={{ component: Shell }} />, root);
        expect(root.textContent).toContain("shell");
        expect(root.textContent).toContain("public home");
    });
});
