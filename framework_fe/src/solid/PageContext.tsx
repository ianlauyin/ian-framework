import type {JSX} from "@solidjs/web";
import {type Context, createContext, createStore, onSettled, type Store, type StoreSetter, useContext} from "solid-js";

export type PageAction<P> = Omit<P, "action" | "provide" | "state">;

export interface Page<P extends PageContext<any>> {
    readonly state: P["state"];
    readonly action: PageAction<P>;
    readonly provide: (children: () => JSX.Element) => JSX.Element;
}

export function register<P extends PageContext<any>>(page: P): Page<P> {
    return {
        state: page.state,
        provide: page.provide,
        get action() {
            return page.action;
        },
    };
}

export abstract class PageContext<State extends object> {
    private readonly context: Context<this> = createContext<this>();
    readonly state: Store<State>;
    protected readonly setState: StoreSetter<State>;

    constructor(initialState: State extends Function ? never : State) {
        [this.state, this.setState] = createStore<State>(initialState);
    }

    protected async onEnter(): Promise<void> {}

    protected async onTick(_second: number): Promise<void> {}

    protected async onLeave(): Promise<void> {}

    get action(): PageAction<this> {
        return useContext(this.context);
    }

    readonly provide = (children: () => JSX.Element): JSX.Element => {
        onSettled(() => {
            void this.onEnter();
            let second = 0;
            const id = this.onTick === PageContext.prototype.onTick ? undefined : setInterval(() => void this.onTick(++second), 1000);
            return () => {
                clearInterval(id);
                void this.onLeave();
            };
        });
        const Provider = this.context;
        return <Provider value={this}>{children()}</Provider>;
    };
}
