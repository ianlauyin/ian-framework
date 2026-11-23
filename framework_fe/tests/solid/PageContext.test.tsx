import {afterEach, describe, expect, it} from "vitest";
import {render} from "@solidjs/web";
import {flush} from "solid-js";
import {PageContext} from "../../src/solid/PageContext";

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
    const page = props.page.use;
    return <p>name {page.name}</p>;
}

let dispose: (() => void) | undefined;

afterEach(() => {
    dispose?.();
    dispose = undefined;
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
});
