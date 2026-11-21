import {createStore, type Store, type StoreSetter} from "solid-js";

export abstract class Module<State extends object> {
    private readonly state: Store<State>;
    protected readonly setState: StoreSetter<State>;

    constructor(initialState: State extends Function ? never : State) {
        [this.state, this.setState] = createStore<State>(initialState);
    }

    store(): Store<State> {
        return this.state;
    }
}
