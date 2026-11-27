import {createStore, type Store, type StoreSetter} from "solid-js";

const modules: Partial<ModuleMap> = {};

export interface ModuleMap {}

export function registerModule<K extends keyof ModuleMap>(name: K, module: ModuleMap[K]): void {
    if (modules[name] !== undefined) throw new Error(`module ${String(name)} is already registered`);
    modules[name] = module;
    void (module as Module<object>).onInit();
}

export function moduleMap(): ModuleMap {
    return modules as ModuleMap;
}

export abstract class Module<State extends object> {
    readonly state: Store<State>;
    protected readonly setState: StoreSetter<State>;

    constructor(initialState: State extends Function ? never : State) {
        [this.state, this.setState] = createStore<State>(initialState);
    }

    async onInit(): Promise<void> {}
}
