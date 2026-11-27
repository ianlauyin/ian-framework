import {describe, expect, it} from "vitest";
import {flush} from "solid-js";
import {Module, PageContext, registerModule} from "../../src/index";

class CounterModule extends Module<{count: number}> {
    readonly events: string[] = [];

    constructor() {
        super({count: 0});
    }

    add() {
        this.setState(state => {
            state.count += 1;
        });
    }

    override async onInit() {
        this.events.push("init");
    }
}

declare module "../../src/index" {
    interface ModuleMap {
        counter: CounterModule;
    }
}

class CounterPage extends PageContext<{}> {
    protected readonly name = "counter";

    constructor() {
        super({});
    }

    get count() {
        return this.module.counter.state.count;
    }

    add() {
        this.module.counter.add();
    }
}

const counter = new CounterModule();

describe("Module", () => {
    it("throws when the page reads a module that is not registered", () => {
        const page = new CounterPage();
        expect(() => page.count).toThrow();
    });

    it("calls onInit when the module is registered", () => {
        registerModule("counter", counter);
        expect(counter.events).toEqual(["init"]);
    });

    it("throws when a name is registered again", () => {
        expect(() => registerModule("counter", new CounterModule())).toThrow();
    });

    it("gives the module to the page and shows a write", () => {
        const page = new CounterPage();
        expect(page.count).toBe(0);

        page.add();
        flush();
        expect(page.count).toBe(1);
    });
});
