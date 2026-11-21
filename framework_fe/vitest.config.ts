import solidPlugin from "@solidjs/vite-plugin";
import {defineConfig} from "vitest/config";

export default defineConfig({
    plugins: [solidPlugin({ssr: false})],
    test: {
        projects: [
            {
                extends: true,
                test: {
                    name: "solid",
                    environment: "happy-dom",
                    include: ["tests/solid/**/*.tsx"],
                },
            },
            {
                extends: true,
                test: {
                    name: "node",
                    environment: "node",
                    include: ["tests/**/*.test.ts"],
                },
            },
        ],
    },
});
