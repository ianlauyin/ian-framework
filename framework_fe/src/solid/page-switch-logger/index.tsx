import { useLocation } from "@solidjs/router";
import { createEffect, type JSX, on } from "solid-js";

import { Logger } from "../../logger";

export function PageSwitchLogger(): JSX.Element {
  const location = useLocation();
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
