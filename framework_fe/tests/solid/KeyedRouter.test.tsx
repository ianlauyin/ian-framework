import {afterEach, describe, expect, it} from "vitest";
import {type RouteSectionProps, useNavigate} from "@solidjs/router";
import {render} from "@solidjs/web";
import {createSignal, flush} from "solid-js";

import {async} from "../../src/solid/async";
import {KeyedRouter, type KeyedRoutes} from "../../src/solid/KeyedRouter";

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
    navigate = useNavigate();
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

type Key = "user" | "non-user" | "open";

const keyed: KeyedRoutes<Key>[] = [
    {key: "user", routes: [{path: "/", component: Layout, children: [{path: "/", component: Home}]}]},
    {key: "non-user", routes: [{path: "/login", component: Login}]},
    {key: "open", routes: [{path: "/landing", component: Landing}]},
];

function activeKeys(signedIn: boolean): Key[] {
    return signedIn ? ["user", "open"] : ["non-user", "open"];
}

function defer<T>() {
    let resolve: (value: T) => void = () => {};
    const promise = new Promise<T>(done => {
        resolve = done;
    });
    return {promise, resolve};
}

function mount(path: string, signedIn: () => boolean): HTMLElement {
    window.history.pushState({}, "", path);
    const root = document.createElement("div");
    dispose = render(() => <KeyedRouter loadingOverlay={<p>loading overlay</p>} activeKeys={activeKeys(signedIn())} keyed={keyed} fallbackPath={signedIn() ? "/" : "/landing"} />, root);
    flush();
    return root;
}

async function settle() {
    await new Promise(resolve => setTimeout(resolve, 20));
    flush();
}

afterEach(() => {
    dispose?.();
    dispose = undefined;
    navigate = undefined;
});

describe("KeyedRouter", () => {
    it("shows a route of an active key", () => {
        const root = mount("/", () => true);
        expect(root.textContent).toContain("layout");
        expect(root.textContent).toContain("home page");
    });

    it("shows a route of a key that is active in each state", () => {
        expect(mount("/landing", () => true).textContent).toContain("landing page");
        dispose?.();
        expect(mount("/landing", () => false).textContent).toContain("landing page");
    });

    it("goes to the fallback path when the key of the route is not active", async () => {
        const root = mount("/login", () => true);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("login page");
    });

    it("goes to the fallback path when no route matches the path", async () => {
        const root = mount("/missing", () => false);
        await settle();
        expect(window.location.pathname).toBe("/landing");
        expect(root.textContent).toContain("landing page");
    });

    it("renders nothing when the fallback path has no active route", async () => {
        window.history.pushState({}, "", "/missing");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter loadingOverlay={<p>loading overlay</p>} activeKeys={activeKeys(false)} keyed={keyed} fallbackPath="/" />, root);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toBe("");
    });

    it("follows a change of the active keys", async () => {
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

    it("renders nothing and goes to the fallback path when no key is active", async () => {
        window.history.pushState({}, "", "/landing");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter loadingOverlay={<p>loading overlay</p>} activeKeys={[]} keyed={keyed} fallbackPath="/" />, root);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toBe("");
    });

    it("renders nothing and goes to the fallback path when there is no group", async () => {
        window.history.pushState({}, "", "/landing");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter loadingOverlay={<p>loading overlay</p>} activeKeys={[]} keyed={[]} fallbackPath="/" />, root);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toBe("");
    });

    it("keeps the current page in the shared loading boundary across route groups", async () => {
        const landingLoad = defer<typeof Landing>();
        const loginLoad = defer<typeof Login>();
        const lazyKeyed: KeyedRoutes<Key>[] = [
            {key: "user", routes: [{path: "/", component: Layout, children: [{path: "/", component: Home}]}]},
            {key: "non-user", routes: [{path: "/login", component: async(() => loginLoad.promise)}]},
            {key: "open", routes: [{path: "/landing", component: async(() => landingLoad.promise)}]},
        ];
        window.history.pushState({}, "", "/landing");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter loadingOverlay={<p>loading overlay</p>} activeKeys={activeKeys(false)} keyed={lazyKeyed} fallbackPath="/landing" />, root);
        flush();

        landingLoad.resolve(Landing);
        await settle();
        expect(root.textContent).toContain("landing page");
        expect(root.textContent).not.toContain("loading overlay");

        navigate?.("/login");
        await settle();
        const faded = [...root.querySelectorAll("div")].find(element => element.style.opacity === "0.5");
        expect(faded?.textContent).toContain("landing page");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("login page");

        loginLoad.resolve(Login);
        await settle();
        expect(root.textContent).toContain("login page");
        expect(root.textContent).not.toContain("landing page");
        expect(root.textContent).not.toContain("loading overlay");
    });

    it("navigates between routes", async () => {
        const root = mount("/login", () => false);
        navigate?.("/landing");
        await settle();
        expect(root.textContent).toContain("landing page");
    });
});
