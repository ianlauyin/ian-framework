import type {JSX} from "@solidjs/web";
import {type Context, createContext, createStore, type Store, type StoreSetter, useContext} from "solid-js";

export abstract class PageContext<State extends object> {
    private readonly context: Context<this> = createContext<this>();
    protected readonly state: Store<State>;
    protected readonly setState: StoreSetter<State>;

    constructor(initialState: State extends Function ? never : State) {
        [this.state, this.setState] = createStore<State>(initialState);
    }

    get use(): Omit<this, "use" | "provide"> {
        return useContext(this.context);
    }

    get store(): Store<State> {
        return this.state;
    }

    provide(children: () => JSX.Element): JSX.Element {
        const Provider = this.context;
        return <Provider value={this}>{children()}</Provider>;
    }
}
