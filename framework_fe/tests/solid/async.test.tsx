import {afterEach, describe, expect, it} from "vitest";
import {createRouter, type RouteSectionProps, useNavigate} from "@solidjs/router";
import {render} from "@solidjs/web";
import {flush} from "solid-js";

import {async, AsyncContent} from "../../src/solid/async";

let dispose: (() => void) | undefined;
let navigate: ((path: string) => void) | undefined;

function Home() {
    return <p>home page</p>;
}

function Account() {
    return <p>account page</p>;
}

function AccountCenter(props: {id: string}) {
    return <p>center {props.id}</p>;
}

function defer<T>() {
    let resolve: (value: T) => void = () => {};
    const promise = new Promise<T>(done => {
        resolve = done;
    });
    return {promise, resolve};
}

const loadingDelay = 40;

function Layout(props: RouteSectionProps) {
    navigate = useNavigate();
    return (
        <div>
            <p>chrome</p>
            <AsyncContent loadingOverlay={<p>loading overlay</p>} loadingDelay={loadingDelay}>
                {props.children}
            </AsyncContent>
        </div>
    );
}

function mount(path: string, home: () => Promise<typeof Home>, account: () => Promise<typeof Account>): HTMLElement {
    window.history.pushState({}, "", path);
    const Router = createRouter({
        routes: [
            {
                path: "/",
                component: Layout,
                children: [
                    {path: "/", component: async(home)},
                    {path: "/account", component: async(account)},
                ],
            },
        ],
    });
    const root = document.createElement("div");
    dispose = render(() => <Router />, root);
    flush();
    return root;
}

function fadedOutlet(root: HTMLElement): HTMLElement | undefined {
    return [...root.querySelectorAll("div")].find(element => element.style.opacity === "0.5");
}

function chrome(root: HTMLElement): HTMLElement | undefined {
    return [...root.querySelectorAll("p")].find(element => element.textContent === "chrome");
}

async function settle() {
    await wait(20);
}

async function wait(ms: number) {
    await new Promise(resolve => setTimeout(resolve, ms));
    flush();
}

afterEach(() => {
    dispose?.();
    dispose = undefined;
    navigate = undefined;
    window.history.pushState({}, "", "/");
});

describe("async", () => {
    it("shows the loading overlay for the outlet and keeps the chrome until the named page loads", async () => {
        const home = defer<typeof Home>();
        const root = mount(
            "/",
            () => home.promise,
            () => Promise.resolve(Account)
        );
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("home page");
        expect(fadedOutlet(root)).toBeUndefined();

        await wait(loadingDelay + 20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("home page");
        expect(fadedOutlet(root)).toBeUndefined();

        home.resolve(Home);
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
        expect(root.querySelector("[aria-busy='false']")).toBeTruthy();
    });

    it("keeps the current page and shows the dim overlay after the delay on a slow navigation", async () => {
        const home = defer<typeof Home>();
        const account = defer<typeof Account>();
        const root = mount(
            "/",
            () => home.promise,
            () => account.promise
        );
        home.resolve(Home);
        await settle();
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");

        navigate?.("/account");
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(root.textContent).not.toContain("account page");
        expect(fadedOutlet(root)).toBeUndefined();
        expect(root.querySelector("[aria-busy='true']")).toBeNull();

        await wait(loadingDelay);
        const faded = fadedOutlet(root);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("account page");
        expect(faded?.textContent).toContain("home page");
        expect(faded?.style.pointerEvents).toBe("none");
        expect(faded?.contains(chrome(root)!)).toBe(false);
        expect(chrome(root)?.style.opacity).not.toBe("0.5");
        expect(faded?.parentElement?.getAttribute("aria-busy")).toBe("true");

        account.resolve(Account);
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
        expect(root.querySelector("[aria-busy='true']")).toBeNull();
        expect(root.querySelector("[aria-busy='false']")).toBeTruthy();
    });

    it("does not show the overlay on a navigation that is already loaded", async () => {
        const root = mount(
            "/",
            () => Promise.resolve(Home),
            () => Promise.resolve(Account)
        );
        await settle();
        expect(root.textContent).toContain("home page");

        navigate?.("/account");
        await settle();
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();

        await wait(loadingDelay + 20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
        expect(root.querySelector("[aria-busy='true']")).toBeNull();
    });

    it("shows a ready outlet and keeps the chrome", async () => {
        const root = mount(
            "/account",
            () => Promise.resolve(Home),
            () => Promise.resolve(Account)
        );
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
    });

    it("renders a named page that receives props", async () => {
        const Center = async(() => Promise.resolve(AccountCenter));
        window.history.pushState({}, "", "/");
        const Router = createRouter({
            routes: [
                {
                    path: "/",
                    component: () => (
                        <AsyncContent loadingOverlay={<p>loading overlay</p>}>
                            <Center id="7" />
                        </AsyncContent>
                    ),
                },
            ],
        });
        const root = document.createElement("div");
        dispose = render(() => <Router />, root);
        await settle();
        expect(root.textContent).toContain("center 7");
        expect(root.textContent).not.toContain("loading overlay");
    });
});
