import {afterEach, describe, expect, it, vi} from "vitest";

import {sleep} from "../../src/util/sleep";

describe("sleep", () => {
    afterEach(() => {
        vi.useRealTimers();
    });

    it("resolves after the time", async () => {
        vi.useFakeTimers();
        const done = vi.fn();
        void sleep(100).then(done);

        await vi.advanceTimersByTimeAsync(99);
        expect(done).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(1);
        expect(done).toHaveBeenCalledOnce();
    });
});
