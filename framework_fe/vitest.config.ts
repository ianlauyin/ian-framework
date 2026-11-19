import solidPlugin from "@solidjs/vite-plugin";
import { defineConfig } from "vitest/config";

export default defineConfig({
    plugins: [solidPlugin({ ssr: false })],
    test: {
        projects: [
            {
                extends: true,
                test: {
                    name: "solid",
                    environment: "happy-dom",
                    include: ["test/solid/**/*.tsx"],
                },
            },
            {
                extends: true,
                test: {
                    name: "node",
                    environment: "node",
                    include: ["test/**/*.test.ts"],
                },
            },
        ],
    },
});
