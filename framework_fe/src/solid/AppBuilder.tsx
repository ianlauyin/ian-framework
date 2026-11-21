import { type JSX, render } from "@solidjs/web";
import { createStore, type Store, type StoreSetter } from "solid-js";
import { Logger } from "../logger";
import { ErrorBoundary, type ErrorBoundaryProps } from "./ErrorBoundary";

export abstract class AppBuilder<State extends object> {
    private readonly state: Store<State>;
    protected readonly setState: StoreSetter<State>;
    private readonly errorFallback: ErrorBoundaryProps["fallback"];

    constructor(initialState: State extends Function ? never : State, errorFallback: ErrorBoundaryProps["fallback"]) {
        [this.state, this.setState] = createStore<State>(initialState);
        this.errorFallback = errorFallback;
    }

    protected abstract onStarting: () => Promise<void>;

    store(): Store<State> {
        return this.state;
    }

    protected start(root: HTMLElement, children: () => JSX.Element): () => void {
        Logger.capture("APP/start", () => this.onStarting()).catch(() => {});
        const dispose = render(() => <ErrorBoundary fallback={this.errorFallback}>{children()}</ErrorBoundary>, root);
        return dispose;
    }
}
