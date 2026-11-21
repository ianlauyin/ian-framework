import { type RouteSectionProps, useNavigate } from "@solidjs/router";
import { render } from "@solidjs/web";
import { createSignal, flush } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";

import { type AccessRoute, AccessRouter } from "../../src/solid/AccessRouter";

let dispose: (() => void) | undefined;
let navigate: ((path: string) => void) | undefined;

function Home() {
    return <p>home page</p>;
}

function Login() {
    navigate = useNavigate();
    return <p>login page</p>;
}

function Landing() {
    return <p>landing page</p>;
}

function Layout(props: RouteSectionProps) {
    return (
        <div>
            <p>layout</p>
            {props.children}
        </div>
    );
}

function routes(signedIn: () => boolean): AccessRoute[] {
    return [
        { path: "/", component: Layout, children: [{ path: "/", component: Home }], canAccess: signedIn },
        { path: "/login", component: Login, canAccess: () => !signedIn() },
        { path: "/landing", component: Landing },
    ];
}

function mount(path: string, signedIn: () => boolean): HTMLElement {
    window.history.pushState({}, "", path);
    const root = document.createElement("div");
    dispose = render(() => <AccessRouter routes={routes(signedIn)} fallbackPath={signedIn() ? "/" : "/landing"} />, root);
    flush();
    return root;
}

async function settle() {
    await new Promise((resolve) => setTimeout(resolve, 20));
    flush();
}

afterEach(() => {
    dispose?.();
    dispose = undefined;
    navigate = undefined;
});

describe("AccessRouter", () => {
    it("shows a route that canAccess allows", () => {
        const root = mount("/", () => true);
        expect(root.textContent).toContain("layout");
        expect(root.textContent).toContain("home page");
    });

    it("shows a route that has no canAccess", () => {
        expect(mount("/landing", () => true).textContent).toContain("landing page");
        dispose?.();
        expect(mount("/landing", () => false).textContent).toContain("landing page");
    });

    it("goes to the fallback when canAccess denies the route", async () => {
        const root = mount("/login", () => true);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("login page");
    });

    it("goes to the fallback when no route matches the path", async () => {
        const root = mount("/missing", () => false);
        await settle();
        expect(window.location.pathname).toBe("/landing");
        expect(root.textContent).toContain("landing page");
    });

    it("renders nothing when the fallback has no accessible route", async () => {
        window.history.pushState({}, "", "/missing");
        const root = document.createElement("div");
        dispose = render(() => <AccessRouter routes={routes(() => false)} fallbackPath="/" />, root);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toBe("");
    });

    it("follows a change of canAccess", async () => {
        const [signedIn, setSignedIn] = createSignal(false);
        const root = mount("/login", signedIn);
        expect(root.textContent).toContain("login page");
        setSignedIn(true);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toContain("home page");
        setSignedIn(false);
        await settle();
        expect(window.location.pathname).toBe("/landing");
        expect(root.textContent).toContain("landing page");
    });

    it("navigates between routes", async () => {
        const root = mount("/login", () => false);
        navigate?.("/landing");
        await settle();
        expect(root.textContent).toContain("landing page");
    });
});
