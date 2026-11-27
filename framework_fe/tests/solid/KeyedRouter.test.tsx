import {afterEach, describe, expect, it} from "vitest";
import {type RouteSectionProps, useNavigate} from "@solidjs/router";
import {render} from "@solidjs/web";
import {createSignal, flush} from "solid-js";

import {async} from "../../src/solid/async";
import {KeyedRouter, type KeyedRoutes, preloadKeyedRoutes} from "../../src/solid/KeyedRouter";

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

type Key = "user" | "non-user" | "open";

const keyed: KeyedRoutes<Key>[] = [
    {key: "user", routes: [{path: "/", component: Layout, children: [{path: "/", component: Home}]}]},
    {key: "non-user", routes: [{path: "/login", component: Login}]},
    {key: "open", routes: [{path: "/landing", component: Landing}]},
];

function activeKeys(signedIn: boolean): Key[] {
    return signedIn ? ["user", "open"] : ["non-user", "open"];
}

function mount(path: string, signedIn: () => boolean): HTMLElement {
    window.history.pushState({}, "", path);
    const root = document.createElement("div");
    dispose = render(() => <KeyedRouter activeKeys={activeKeys(signedIn())} keyed={keyed} fallbackPath={signedIn() ? "/" : "/landing"} loadingOverlay={<p>loading overlay</p>} />, root);
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
        dispose = render(() => <KeyedRouter activeKeys={activeKeys(false)} keyed={keyed} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} />, root);
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
        dispose = render(() => <KeyedRouter activeKeys={[]} keyed={keyed} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} />, root);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toBe("");
    });

    it("renders nothing and goes to the fallback path when there is no group", async () => {
        window.history.pushState({}, "", "/landing");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={[]} keyed={[]} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} />, root);
        await settle();
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toBe("");
    });

    it("navigates between routes", async () => {
        const root = mount("/login", () => false);
        navigate?.("/landing");
        await settle();
        expect(root.textContent).toContain("landing page");
    });
});

function defer<T>() {
    let resolve: (value: T) => void = () => {};
    const promise = new Promise<T>(done => {
        resolve = done;
    });
    return {promise, resolve};
}

function fadedOutlet(root: HTMLElement): HTMLElement | undefined {
    return [...root.querySelectorAll("div")].find(element => element.style.opacity === "0.5");
}

async function wait(ms: number) {
    await new Promise(resolve => setTimeout(resolve, ms));
    flush();
}

describe("KeyedRouter loading", () => {
    const delay = 40;

    function Chrome(props: RouteSectionProps) {
        navigate = useNavigate();
        return (
            <div>
                <p>chrome</p>
                {props.children}
            </div>
        );
    }

    function Page(text: string) {
        return () => <p>{text}</p>;
    }

    it("shows the overlay on the first load and keeps the chrome undimmed", async () => {
        const home = defer<ReturnType<typeof Page>>();
        const HomePage = async(() => home.promise);
        const routes: KeyedRoutes<"user">[] = [{key: "user", routes: [{path: "/", component: Chrome, children: [{path: "/", component: HomePage}]}]}];
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={["user"]} keyed={routes} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} loadingDelay={delay} />, root);
        flush();
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("home page");
        expect(fadedOutlet(root)).toBeUndefined();

        home.resolve(Page("home page"));
        await wait(20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
    });

    it("keeps the current page and dims only the outlet after the delay", async () => {
        const account = defer<ReturnType<typeof Page>>();
        const AccountPage = async(() => account.promise);
        const routes: KeyedRoutes<"user">[] = [
            {
                key: "user",
                routes: [
                    {
                        path: "/",
                        component: Chrome,
                        children: [
                            {path: "/", component: Page("home page")},
                            {path: "/account", component: AccountPage},
                        ],
                    },
                ],
            },
        ];
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={["user"]} keyed={routes} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} loadingDelay={delay} />, root);
        flush();
        expect(root.textContent).toContain("home page");

        navigate?.("/account");
        await wait(20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();

        await wait(delay);
        const faded = fadedOutlet(root);
        const chrome = [...root.querySelectorAll("p")].find(element => element.textContent === "chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("account page");
        expect(faded?.textContent).toContain("home page");
        expect(faded?.contains(chrome!)).toBe(false);
        expect(faded?.style.pointerEvents).toBe("none");
        expect(faded?.parentElement?.getAttribute("aria-busy")).toBe("true");

        account.resolve(Page("account page"));
        await wait(20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
    });

    it("keeps the layout page while a page outside the layout loads", async () => {
        const landing = defer<ReturnType<typeof Page>>();
        const LandingPage = async(() => landing.promise);
        const routes: KeyedRoutes<"user" | "open">[] = [
            {key: "user", routes: [{path: "/", component: Chrome, children: [{path: "/", component: Page("home page")}]}]},
            {key: "open", routes: [{path: "/landing", component: LandingPage}]},
        ];
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={["user", "open"]} keyed={routes} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} loadingDelay={delay} />, root);
        flush();
        navigate?.("/landing");
        await wait(20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");

        await wait(delay);
        const faded = fadedOutlet(root);
        const chrome = [...root.querySelectorAll("p")].find(element => element.textContent === "chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).toContain("loading overlay");
        expect(root.textContent).not.toContain("landing page");
        expect(faded?.contains(chrome!)).toBe(false);

        landing.resolve(Page("landing page"));
        await wait(20);
        expect(root.textContent).toContain("landing page");
        expect(root.textContent).not.toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");
    });

    it("does not show the overlay when the next page is already loaded", async () => {
        const routes: KeyedRoutes<"user">[] = [
            {
                key: "user",
                routes: [
                    {
                        path: "/",
                        component: Chrome,
                        children: [
                            {path: "/", component: Page("home page")},
                            {path: "/account", component: Page("account page")},
                        ],
                    },
                ],
            },
        ];
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={["user"]} keyed={routes} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} loadingDelay={delay} />, root);
        flush();
        navigate?.("/account");
        await wait(20);
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading overlay");
        await wait(delay + 20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
    });

    it("keeps the current page while a nested page loads", async () => {
        const detail = defer<ReturnType<typeof Page>>();
        const Detail = async(() => detail.promise);
        function Inner(props: RouteSectionProps) {
            return (
                <div>
                    <p>inner chrome</p>
                    {props.children}
                </div>
            );
        }
        const routes: KeyedRoutes<"user">[] = [
            {
                key: "user",
                routes: [
                    {
                        path: "/",
                        component: Chrome,
                        children: [
                            {path: "/", component: Page("home page")},
                            {path: "/account", component: Inner, children: [{path: "/", component: Detail}]},
                        ],
                    },
                ],
            },
        ];
        window.history.pushState({}, "", "/");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={["user"]} keyed={routes} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} loadingDelay={delay} />, root);
        flush();
        navigate?.("/account");
        await wait(delay + 20);
        const overlays = [...root.querySelectorAll("p")].filter(element => element.textContent === "loading overlay");
        const faded = fadedOutlet(root);
        const chrome = [...root.querySelectorAll("p")].find(element => element.textContent === "chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("inner chrome");
        expect(overlays).toHaveLength(1);
        expect(faded?.contains(chrome!)).toBe(false);

        detail.resolve(Page("account page"));
        await wait(20);
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("inner chrome");
        expect(root.textContent).toContain("account page");
        expect(root.textContent).not.toContain("loading overlay");
        expect(fadedOutlet(root)).toBeUndefined();
    });

    it("keeps the old screen until a key change finishes preloading", async () => {
        const home = defer<ReturnType<typeof Page>>();
        let loads = 0;
        const HomePage = async(() => {
            loads++;
            return home.promise;
        });
        const routes: KeyedRoutes<"user" | "non-user">[] = [
            {key: "user", routes: [{path: "/", component: Chrome, children: [{path: "/", component: HomePage}]}]},
            {key: "non-user", routes: [{path: "/login", component: Page("login page")}]},
        ];
        const [signedIn, setSignedIn] = createSignal(false);
        window.history.pushState({}, "", "/login");
        const root = document.createElement("div");
        dispose = render(() => <KeyedRouter activeKeys={signedIn() ? ["user"] : ["non-user"]} keyed={routes} fallbackPath="/" loadingOverlay={<p>loading overlay</p>} loadingDelay={delay} />, root);
        flush();
        expect(root.textContent).toContain("login page");

        setSignedIn(true);
        flush();
        expect(loads).toBe(1);
        expect(window.location.pathname).toBe("/login");
        expect(root.textContent).toContain("login page");
        expect(root.textContent).not.toContain("home page");
        expect(root.textContent).not.toContain("loading overlay");

        await wait(delay + 20);
        expect(window.location.pathname).toBe("/login");
        expect(root.textContent).toContain("login page");
        expect(root.textContent).not.toContain("loading overlay");

        home.resolve(Page("home page"));
        await wait(20);
        expect(window.location.pathname).toBe("/");
        expect(root.textContent).toContain("chrome");
        expect(root.textContent).toContain("home page");
        expect(root.textContent).not.toContain("login page");
        expect(root.textContent).not.toContain("loading overlay");
    });
});

describe("preloadKeyedRoutes", () => {
    function Page(text: string) {
        return () => <p>{text}</p>;
    }

    it("preloads the matched branch and the fallback path", async () => {
        let homeLoads = 0;
        let accountLoads = 0;
        let landingLoads = 0;
        let newLoads = 0;
        let detailLoads = 0;
        let layoutLoads = 0;
        const HomePage = async(() => {
            homeLoads++;
            return Promise.resolve(Page("home"));
        });
        const AccountPage = async(() => {
            accountLoads++;
            return Promise.resolve(Page("account"));
        });
        const LandingPage = async(() => {
            landingLoads++;
            return Promise.resolve(Page("landing"));
        });
        const NewPage = async(() => {
            newLoads++;
            return Promise.resolve(Page("new"));
        });
        const DetailPage = async(() => {
            detailLoads++;
            return Promise.resolve(Page("detail"));
        });
        const LayoutPage = async(() => {
            layoutLoads++;
            return Promise.resolve(Page("layout"));
        });
        const keyed: KeyedRoutes<"user" | "open">[] = [
            {
                key: "user",
                routes: [
                    {
                        path: "/",
                        component: LayoutPage,
                        children: [
                            {path: "/", component: HomePage},
                            {path: "/account/new", component: NewPage},
                            {path: "/account/:id", component: DetailPage},
                            {path: "/account", component: AccountPage},
                        ],
                    },
                ],
            },
            {key: "open", routes: [{path: "/landing", component: LandingPage}]},
        ];
        const source = {activeKeys: ["user", "open"] as const, keyed, fallbackPath: "/landing"};

        await preloadKeyedRoutes(source, "/account");
        expect(layoutLoads).toBe(1);
        expect(accountLoads).toBe(1);
        expect(homeLoads).toBe(0);
        expect(landingLoads).toBe(0);
        expect(newLoads).toBe(0);
        expect(detailLoads).toBe(0);

        await preloadKeyedRoutes(source, "/account/new");
        expect(newLoads).toBe(1);
        expect(detailLoads).toBe(0);

        await preloadKeyedRoutes(source, "/account/7");
        expect(detailLoads).toBe(1);
        expect(newLoads).toBe(1);

        await preloadKeyedRoutes(source, "/missing");
        expect(landingLoads).toBe(1);
        expect(homeLoads).toBe(0);
        expect(layoutLoads).toBe(1);

        window.history.pushState({}, "", "/account");
        await preloadKeyedRoutes(source);
        expect(accountLoads).toBe(1);
    });
});
