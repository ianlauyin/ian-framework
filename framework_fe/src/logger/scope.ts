import type { LogFields, LogScope } from "./types";

export class ActionScope implements LogScope {
  private readonly info: LogFields = {};
  private closed = false;

  addInfo(key: string, value: string | number | boolean): void {
    if (this.closed) {
      console.warn(`scope.addInfo(${key}) is ignored, because the action is out of its scope`);
    } else {
      this.info[key] = value;
    }
  }

  close(): LogFields {
    this.closed = true;
    return this.info;
  }
}

export const OUT_OF_SCOPE = new ActionScope();
OUT_OF_SCOPE.close();
