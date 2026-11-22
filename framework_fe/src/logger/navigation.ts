export function watchNavigation(report: (url: string) => void): void {
    const navigated = () => report(window.location.href);
    navigated();
    const history = window.history;
    const pushState = history.pushState.bind(history);
    const replaceState = history.replaceState.bind(history);
    history.pushState = (...args) => {
        pushState(...args);
        navigated();
    };
    history.replaceState = (...args) => {
        replaceState(...args);
        navigated();
    };
    window.addEventListener("popstate", navigated);
}
