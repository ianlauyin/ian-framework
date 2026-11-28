import type {JSX} from "@solidjs/web";
import {type Accessor, type Context, createContext, createSignal, createStore, type Setter, onSettled, type Store, type StoreSetter, useContext} from "solid-js";
import {Logger} from "../logger";
import {type ModuleMap, moduleMap} from "./Module";

type Fn = (...args: any[]) => any;
type Own<P> = Exclude<keyof P, "action" | "data" | "provide" | "state" | "loading">;

export type PageAction<P> = {[K in Own<P> as P[K] extends Fn ? K : never]: P[K]};

export type PageData<P> = {[K in Own<P> as P[K] extends Fn ? never : K]: P[K]};

export interface Page<P extends PageContext<any>> {
    readonly state: P["state"];
    readonly data: PageData<P>;
    readonly loading: boolean;
    readonly action: PageAction<P>;
    readonly provide: (children: () => JSX.Element) => JSX.Element;
}

export function registerPage<P extends PageContext<any>>(page: P): Page<P> {
    return {
        state: page.state,
        data: page as PageData<P>,
        provide: page.provide,
        get loading() {
            return page.loading;
        },
        get action() {
            return page.action;
        },
    };
}

export abstract class PageContext<State extends object> {
    protected abstract readonly name: string;
    protected tickInterval: number = 60_000;
    readonly state: Store<State>;
    protected readonly setState: StoreSetter<State>;
    private readonly loadingSignal: Accessor<boolean>;
    private readonly setLoadingSignal: Setter<boolean>;
    private readonly context: Context<this> = createContext<this>();

    constructor(initialState: State extends Function ? never : State) {
        [this.state, this.setState] = createStore<State>(initialState);
        [this.loadingSignal, this.setLoadingSignal] = createSignal(false);
    }

    protected get modules(): ModuleMap {
        return moduleMap();
    }

    protected async onEnter(): Promise<void> {}

    protected async onTick(): Promise<void> {}

    protected async onLeave(): Promise<void> {}

    protected async withLoading<T>(work: () => Promise<T>): Promise<T> {
        this.setLoadingSignal(true);
        try {
            return await work();
        } finally {
            this.setLoadingSignal(false);
        }
    }

    get loading(): boolean {
        return this.loadingSignal();
    }

    get action(): PageAction<this> {
        return useContext(this.context);
    }

    readonly provide = (children: () => JSX.Element): JSX.Element => {
        onSettled(() => {
            this.log("onEnter", () => this.onEnter());
            const id = this.onTick === PageContext.prototype.onTick ? undefined : setInterval(() => this.log("onTick", () => this.onTick()), this.tickInterval);
            return () => {
                clearInterval(id);
                this.log("onLeave", () => this.onLeave());
            };
        });
        const Provider = this.context;
        return <Provider value={this}>{children()}</Provider>;
    };

    private log(hook: string, run: () => Promise<void>): void {
        Logger.capture(`${this.name}.${hook}`, run).catch(() => {});
    }
}
