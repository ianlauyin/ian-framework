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
            <PageContent fallback={<p>loading page</p>}>{props.children}</PageContent>
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
    it("shows the fallback for the outlet and keeps the chrome until the named page loads", async () => {
        const home = defer<typeof Home>();
        const root = mount(
            "/",
            () => home.promise,
            () => Promise.resolve(Account)
        );
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("loading page");
        expect(root.textContent).not.toContain("home page");

        home.resolve(Home);
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading page");
    });

    it("shows the fallback again when the pathname changes and keeps the chrome", async () => {
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
        expect(root.textContent).not.toContain("loading page");

        navigate?.("/account");
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("loading page");
        expect(root.textContent).not.toContain("home page");
        expect(root.textContent).not.toContain("account page");

        account.resolve(Account);
        await settle();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading page");
        expect(root.textContent).not.toContain("home page");
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
        expect(root.textContent).not.toContain("loading page");
    });

    it("renders a named page that receives props", async () => {
        const Center = page(() => Promise.resolve(AccountCenter));
        window.history.pushState({}, "", "/");
        const Router = createRouter({
            routes: [
                {
                    path: "/",
                    component: () => (
                        <PageContent fallback={<p>loading page</p>}>
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
        expect(root.textContent).not.toContain("loading page");
    });
});
