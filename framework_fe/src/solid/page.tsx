import {useLocation} from "@solidjs/router";
import type {JSX} from "@solidjs/web";
import {type Component, lazy, Loading as Suspense} from "solid-js";

export interface PageContentProps {
    children?: JSX.Element;
    fallback: JSX.Element;
}

export function page<T extends Component<any>>(load: () => Promise<T>): T {
    return lazy(() => load().then(component => ({default: component})));
}

export function PageContent(props: PageContentProps): JSX.Element {
    const location = useLocation();
    return (
        <Suspense fallback={props.fallback} on={location.pathname}>
            {props.children}
        </Suspense>
    );
}
