import {afterEach, describe, expect, it, vi} from "vitest";
import {render} from "@solidjs/web";
import {flush} from "solid-js";
import {Logger} from "../../src/logger";
import {PageContext, registerPage} from "../../src/solid/PageContext";

class TestPage extends PageContext<{name: string}> {
    constructor(name: string) {
        super({name});
    }

    get name() {
        return this.state.name;
    }

    rename(name: string) {
        this.setState(state => {
            state.name = name;
        });
    }
}

function Name(props: {page: TestPage}) {
    const page = props.page.action;
    return <p>name {page.name}</p>;
}

class LifecyclePage extends PageContext<{events: string[]}> {
    protected readonly name = "lifecycle";
    readonly events: string[] = [];

    constructor(tickInterval = 1000) {
        super({events: []});
        this.tickInterval = tickInterval;
    }

    protected override async onEnter() {
        this.events.push("enter");
    }

    protected override async onTick() {
        this.events.push("tick");
    }

    protected override async onLeave() {
        this.events.push("leave");
    }
}

class DefaultTickPage extends PageContext<{}> {
    protected readonly name = "default";
    ticks = 0;

    constructor() {
        super({});
    }

    protected override async onTick() {
        this.ticks++;
        throw new Error("tick failed");
    }
}

let dispose: (() => void) | undefined;

afterEach(() => {
    dispose?.();
    dispose = undefined;
    vi.useRealTimers();
    vi.restoreAllMocks();
});

describe("PageContext", () => {
    it("gives the page to the children and shows a write", () => {
        const page = new TestPage("first");
        const root = document.createElement("div");
        dispose = render(() => page.provide(() => <Name page={page} />), root);
        expect(root.textContent).toContain("name first");

        page.rename("second");
        flush();
        expect(root.textContent).toContain("name second");
    });

    it("gives the outer page in a nested page", () => {
        const outer = new TestPage("outer");
        const inner = new TestPage("inner");
        const root = document.createElement("div");
        dispose = render(
            () =>
                outer.provide(() =>
                    inner.provide(() => (
                        <>
                            <Name page={outer} />
                            <Name page={inner} />
                        </>
                    ))
                ),
            root
        );
        expect(root.textContent).toContain("name outer");
        expect(root.textContent).toContain("name inner");
    });

    it("throws when the reader is not in the page", () => {
        const page = new TestPage("first");
        const other = new TestPage("other");
        const root = document.createElement("div");
        expect(() => {
            dispose = render(() => other.provide(() => <Name page={page} />), root);
        }).toThrow();
    });

    it("gives the page through registerPage", () => {
        const page = registerPage(new TestPage("first"));
        function RegisteredName() {
            const action = page.action;
            return (
                <p>
                    name {action.name} state {page.state.name}
                </p>
            );
        }
        const root = document.createElement("div");
        dispose = render(() => page.provide(() => <RegisteredName />), root);
        expect(root.textContent).toContain("name first state first");

        expect(() => page.action).toThrow();
    });

    it("calls onTick each tickInterval", () => {
        vi.useFakeTimers();
        const page = new LifecyclePage(500);
        const root = document.createElement("div");
        dispose = render(() => page.provide(() => <p />), root);
        flush();
        vi.advanceTimersByTime(1000);
        expect(page.events).toEqual(["enter", "tick", "tick"]);
    });

    it("calls onEnter, onTick each second, and onLeave", () => {
        vi.useFakeTimers();
        const page = new LifecyclePage();
        const root = document.createElement("div");
        dispose = render(() => page.provide(() => <p />), root);
        flush();
        expect(page.events).toEqual(["enter"]);

        vi.advanceTimersByTime(2000);
        expect(page.events).toEqual(["enter", "tick", "tick"]);

        dispose();
        dispose = undefined;
        vi.advanceTimersByTime(1000);
        expect(page.events).toEqual(["enter", "tick", "tick", "leave"]);
    });

    it("logs each hook with the page name", () => {
        vi.useFakeTimers();
        const capture = vi.spyOn(Logger, "capture");
        const page = new LifecyclePage();
        const root = document.createElement("div");
        dispose = render(() => page.provide(() => <p />), root);
        flush();
        vi.advanceTimersByTime(1000);
        dispose();
        dispose = undefined;

        expect(capture.mock.calls.map(call => call[0])).toEqual(["lifecycle.onEnter", "lifecycle.onTick", "lifecycle.onLeave"]);
    });

    it("calls onTick each minute by default and catches a hook error after the log", async () => {
        vi.useFakeTimers();
        const page = new DefaultTickPage();
        const root = document.createElement("div");
        dispose = render(() => page.provide(() => <p />), root);
        flush();

        vi.advanceTimersByTime(59_999);
        expect(page.ticks).toBe(0);
        vi.advanceTimersByTime(1);
        expect(page.ticks).toBe(1);

        vi.useRealTimers();
        await new Promise(resolve => setTimeout(resolve, 0));
    });
});
