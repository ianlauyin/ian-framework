import type {JSX} from "@solidjs/web";
import {type Context, createContext, createStore, onSettled, type Store, type StoreSetter, useContext} from "solid-js";
import {Logger} from "../logger";
import {type ModuleMap, moduleMap} from "./Module";

export type PageAction<P> = Omit<P, "action" | "provide" | "state">;

export interface Page<P extends PageContext<any>> {
    readonly state: P["state"];
    readonly action: PageAction<P>;
    readonly provide: (children: () => JSX.Element) => JSX.Element;
}

export function registerPage<P extends PageContext<any>>(page: P): Page<P> {
    return {
        state: page.state,
        provide: page.provide,
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
    private readonly context: Context<this> = createContext<this>();

    constructor(initialState: State extends Function ? never : State) {
        [this.state, this.setState] = createStore<State>(initialState);
    }

    protected get modules(): ModuleMap {
        return moduleMap();
    }

    protected async onEnter(): Promise<void> {}

    protected async onTick(): Promise<void> {}

    protected async onLeave(): Promise<void> {}

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
