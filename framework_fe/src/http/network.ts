export interface ErrorBody {
    message: string;
}

export class ApiError extends Error {
    public readonly code: number;
    public readonly requestURL: string;

    constructor(code: number, errorBody: ErrorBody | null, requestURL = "") {
        super(typeof errorBody?.message === "string" ? errorBody.message : "Unknown error");
        this.code = code;
        this.requestURL = requestURL;
    }
}

export class NetworkConnectionException extends Error {
    public readonly requestURL: string;

    constructor(message: string, requestURL: string) {
        super(message);
        this.requestURL = requestURL;
    }
}

export function networkError(error: unknown, requestURL: string): NetworkConnectionException {
    return new NetworkConnectionException(error instanceof Error ? error.message : String(error), requestURL);
}
