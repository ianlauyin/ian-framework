import { useLocation } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { createEffect } from "solid-js";

import { Logger } from "../logger";

export function NavigationLogger(): JSX.Element {
  const location = useLocation();
  createEffect(
    () => `${window.location.origin}${location.pathname}${location.search}${location.hash}`,
    (requestUrl) => {
      Logger.addContext("requestUrl", Logger.maskText(requestUrl));
    },
  );
  createEffect(
    () => location.pathname,
    (to, from) => {
      if (from === undefined || to === from) return;
      Logger.info("PAGE/switch", { from, to });
    },
  );
  return null;
}
