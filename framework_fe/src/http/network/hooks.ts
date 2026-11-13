export type RequestInterceptor = (request: { readonly headers: Headers }) => void | Promise<void>;

type UnauthorizedHandler = () => void;

let unauthorizedHandler: UnauthorizedHandler | undefined;
let requestInterceptor: RequestInterceptor | undefined;

export function setUnauthorizedHandler(handler?: UnauthorizedHandler) {
  unauthorizedHandler = handler;
}

export function setRequestInterceptor(interceptor?: RequestInterceptor) {
  requestInterceptor = interceptor;
}

export async function interceptRequest(headers: Headers): Promise<void> {
  await requestInterceptor?.({ headers });
}

export function notifyUnauthorized(): void {
  unauthorizedHandler?.();
}
