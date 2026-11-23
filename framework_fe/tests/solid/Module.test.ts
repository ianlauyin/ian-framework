import {describe, expect, it} from "vitest";
import {flush} from "solid-js";
import {Module} from "../../src/solid/Module";

class TestModule extends Module<{count: number}> {
    constructor() {
        super({count: 0});
    }

    add() {
        this.setState(state => {
            state.count += 1;
        });
    }
}

describe("Module", () => {
    it("starts with the initial state", () => {
        const module = new TestModule();
        expect(module.store.count).toBe(0);
    });

    it("shows a write of the extended class in the store", () => {
        const module = new TestModule();
        module.add();
        flush();
        expect(module.store.count).toBe(1);
    });
});
