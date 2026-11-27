import {describe, expect, it, vi} from "vitest";

import {createTheme} from "../../src/solid/theme";

describe("createTheme", () => {
    it("starts with the initial theme", () => {
        const state = createTheme<"light" | "dark">("light");
        expect(state.current()).toBe("light");
    });

    it("gives the new theme after set", () => {
        const state = createTheme<"light" | "dark">("light");
        state.set("dark");
        expect(state.current()).toBe("dark");
    });

    it("calls onChange with the new theme only when the theme changes", () => {
        const onChange = vi.fn();
        const state = createTheme<"light" | "dark">("light", onChange);
        state.set("light");
        expect(onChange).not.toHaveBeenCalled();
        state.set("dark");
        expect(onChange).toHaveBeenCalledExactlyOnceWith("dark");
    });
});
