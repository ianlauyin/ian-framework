import { render } from "@solidjs/web";
import { afterEach, describe, expect, it } from "vitest";

import { SwitchRouter } from "../../src/solid/SwitchRouter";

let dispose: (() => void) | undefined;

function ProtectedHome() {
  return <p>protected home</p>;
}

function PublicHome() {
  return <p>public home</p>;
}

afterEach(() => {
  dispose?.();
  dispose = undefined;
});

describe("SwitchRouter", () => {
  it("shows the protected routes when isProtected is true", () => {
    window.history.pushState({}, "", "/");
    const root = document.createElement("div");
    dispose = render(
      () => (
        <SwitchRouter
          isProtected={true}
          protectedRoutes={[{ path: "/", component: ProtectedHome }]}
          publicRoutes={[{ path: "/", component: PublicHome }]}
          unknown={() => "/missing"}
        />
      ),
      root,
    );
    expect(root.textContent).toContain("protected home");
  });

  it("shows the public routes when isProtected is false", () => {
    window.history.pushState({}, "", "/");
    const root = document.createElement("div");
    dispose = render(
      () => (
        <SwitchRouter
          isProtected={false}
          protectedRoutes={[{ path: "/", component: ProtectedHome }]}
          publicRoutes={[{ path: "/", component: PublicHome }]}
          unknown={() => "/missing"}
        />
      ),
      root,
    );
    expect(root.textContent).toContain("public home");
  });

  it("moves an unknown path to the path from unknown", () => {
    window.history.pushState({}, "", "/no-such-page");
    const seen: boolean[] = [];
    const root = document.createElement("div");
    dispose = render(
      () => (
        <SwitchRouter
          isProtected={false}
          protectedRoutes={[{ path: "/home", component: ProtectedHome }]}
          publicRoutes={[{ path: "/home", component: PublicHome }]}
          unknown={(isProtected) => {
            seen.push(isProtected);
            return "/home";
          }}
        />
      ),
      root,
    );
    expect(seen).toEqual([false]);
    expect(root.textContent).toContain("public home");
  });
});
