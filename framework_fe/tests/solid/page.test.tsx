import {afterEach, describe, expect, it} from "vitest";
import {createRouter, type RouteSectionProps, useNavigate} from "@solidjs/router";
import {render} from "@solidjs/web";
import {flush} from "solid-js";

import {page, PageContent} from "../../src/solid/page";

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

function Layout(props: RouteSectionProps) {
    navigate = useNavigate();
    return (
        <div>
            <p>chrome</p>
            <PageContent loadingOverlay={<p>loading overlay</p>}>{props.children}</PageContent>
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
                    {path: "/", component: page(home)},
                    {path: "/account", component: page(account)},
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
    await new Promise(resolve => setTimeout(resolve, 20));
    flush();
}

afterEach(() => {
    dispose?.();
    dispose = undefined;
    navigate = undefined;
    window.history.pushState({}, "", "/");
});

describe("page", () => {
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

        home.resolve(Home);
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
        expect(root.querySelector("[aria-busy='false']")).toBeTruthy();
    });

    it("keeps the current page, dims the outlet, and shows the loading overlay on a later navigation", async () => {
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
        const faded = fadedOutlet(root);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("account page");
        expect(faded?.textContent).toContain("home page");
        expect(faded?.style.pointerEvents).toBe("none");
        expect(faded?.contains(chrome(root)!)).toBe(false);
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
        const Center = page(() => Promise.resolve(AccountCenter));
        window.history.pushState({}, "", "/");
        const Router = createRouter({
            routes: [
                {
                    path: "/",
                    component: () => (
                        <PageContent loadingOverlay={<p>loading overlay</p>}>
                            <Center id="7" />
                        </PageContent>
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
