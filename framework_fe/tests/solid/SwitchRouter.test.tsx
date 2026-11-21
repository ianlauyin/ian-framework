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

function Inner(props: RouteSectionProps) {
    return (
        <div>
            <p>inner</p>
            {props.children}
        </div>
    );
}

function groups(onMismatch: (key: Area) => void = () => {}): KeyedRoutes<Area>[] {
    return [
        { key: Area.Protected, routes: [{ path: "/", component: ProtectedHome }], onMismatch: () => onMismatch(Area.Protected) },
        { key: Area.Public, routes: [{ path: "/", component: PublicHome }], onMismatch: () => onMismatch(Area.Public) },
        { key: Area.Admin, routes: [{ path: "/", component: AdminHome }], onMismatch: () => onMismatch(Area.Admin) },
    ];
}

function unknown() {}

afterEach(() => {
    dispose?.();
    dispose = undefined;
});

describe("SwitchRouter", () => {
    it("shows the routes for the active key", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Protected} keyed={groups()} unknown={unknown} />, root);
        expect(root.textContent).toContain("protected home");
    });

    it("shows another key from the same route list", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Admin} keyed={groups()} unknown={unknown} />, root);
        expect(root.textContent).toContain("admin home");
    });

    it("follows a new active key", () => {
        window.history.pushState({}, "", "/");
        const [active, setActive] = createSignal(Area.Public);
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={active()} keyed={groups()} unknown={unknown} />, root);
        expect(root.textContent).toContain("public home");
        setActive(Area.Protected);
        flush();
        expect(root.textContent).toContain("protected home");
    });

    it("shows the common route around the active routes", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Public} keyed={groups()} unknown={unknown} common={[{ component: Shell }]} />, root);
        expect(root.textContent).toContain("shell");
        expect(root.textContent).toContain("public home");
    });

    it("nests common routes with the first route outside", () => {
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Public} keyed={groups()} unknown={unknown} common={[{ component: Shell }, { component: Inner }]} />, root);
        const text = root.textContent ?? "";
        expect(text.indexOf("shell")).toBeLessThan(text.indexOf("inner"));
        expect(text.indexOf("inner")).toBeLessThan(text.indexOf("public home"));
    });

    it("calls onMismatch of the active group when no route matches", () => {
        window.history.pushState({}, "", "/missing");
        const mismatched: Area[] = [];
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Public} keyed={groups((key) => mismatched.push(key))} unknown={unknown} common={[{ component: Shell }]} />, root);
        flush();
        expect(mismatched).toEqual([Area.Public]);
        expect(root.textContent).not.toContain("shell");
    });

    it("does not call onMismatch when a route matches", () => {
        window.history.pushState({}, "", "/");
        const mismatched: Area[] = [];
        const root = document.createElement("div");
        dispose = render(() => <SwitchRouter active={Area.Public} keyed={groups((key) => mismatched.push(key))} unknown={unknown} />, root);
        flush();
        expect(mismatched).toEqual([]);
    });

    it("calls unknown when no group has the active key", () => {
        window.history.pushState({}, "", "/");
        let calls = 0;
        const root = document.createElement("div");
        const keyed = groups().filter((group) => group.key !== Area.Admin);
        dispose = render(() => <SwitchRouter active={Area.Admin} keyed={keyed} unknown={() => calls++} />, root);
        flush();
        expect(calls).toBe(1);
        expect(root.textContent).toBe("");
    });
});
