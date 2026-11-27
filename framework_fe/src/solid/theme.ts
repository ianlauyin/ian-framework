import {createStore, flush, untrack} from "solid-js";

export interface ThemeState<Theme extends string> {
    current(): Theme;
    set(theme: Theme): void;
}

export function createTheme<Theme extends string>(initial: NoInfer<Theme>, onChange?: (theme: Theme) => void): ThemeState<Theme> {
    const [state, setState] = createStore({theme: initial as Theme});
    return {
        current: () => state.theme,
        set(theme) {
            if (theme === untrack(() => state.theme)) return;
            setState(draft => {
                draft.theme = theme;
            });
            flush();
            onChange?.(theme);
        },
    };
}
