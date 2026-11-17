import { describe, expect, it } from "vitest";

import { createLocale, type Locales } from "../../src/solid/locale";

type TestDictionary = { action: { save: string; cancel: string }; state: { loading: string } };

const locales: Locales<"en" | "zh-Hant", TestDictionary> = {
  en: { action: { save: "Save", cancel: "Cancel" }, state: { loading: "Loading" } },
  "zh-Hant": { action: { save: "儲存", cancel: "取消" }, state: { loading: "載入中" } },
};

describe("createLocale", () => {
  it("starts with the initial locale", () => {
    const state = createLocale(locales, "en");
    expect(state.current()).toBe("en");
    expect(state.dictionary()).toBe(locales.en);
  });

  it("gives the dictionary of the new locale after changeLocale", () => {
    const state = createLocale(locales, "en");
    state.changeLocale("zh-Hant");
    expect(state.current()).toBe("zh-Hant");
    expect(state.dictionary()).toBe(locales["zh-Hant"]);
    expect(state.dictionary().action.save).toBe("儲存");
  });

  it("keeps the current locale when the new locale has no dictionary", () => {
    const state = createLocale<string, TestDictionary>(locales, "en");
    state.changeLocale("fr");
    expect(state.dictionary()).toBe(locales.en);
    expect(state.current()).toBe("en");
  });
});
