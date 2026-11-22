import {errorWithoutStack, toError} from "./event";

export function watchWindowError(report: (action: string, error: Error) => void): void {
    window.addEventListener(
        "error",
        event => {
            if (event.error instanceof Error) {
                report("WINDOW/error", event.error);
                return;
            }
            const message = event.message || domSourceMessage(event.target) || "window error";
            report("WINDOW/error", errorWithoutStack(message));
        },
        true
    );
    window.addEventListener("unhandledrejection", event => {
        report("WINDOW/unhandledrejection", toError(event.reason));
    });
}

function domSourceMessage(target: EventTarget | null): string | null {
    if (typeof Element === "undefined" || !(target instanceof Element)) return null;
    const source = target.getAttribute("src") ?? target.getAttribute("href") ?? "";
    return `DOM source error: <${target.tagName.toLowerCase()}> ${source}`.trim();
}
