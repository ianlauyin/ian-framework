import { useLocation } from "@solidjs/router";
import { createEffect, type JSX, on } from "solid-js";

import { Logger } from "../../logger";

export function NavigationLogger(): JSX.Element {
  const location = useLocation();
  createEffect(() => {
    const requestUrl = `${window.location.origin}${location.pathname}${location.search}${location.hash}`;
    Logger.addContext("request_url", Logger.maskText(requestUrl));
  });
  createEffect(
    on(
      () => location.pathname,
      (to, from) => {
        if (from === undefined || to === from) return;
        Logger.info("PAGE/switch", { from, to });
      },
    ),
  );
  return null;
}
