import { expect, it, vi } from "vitest";

// i18n.ts picks the language from the browser when it loads.
vi.stubGlobal("localStorage", { getItem: () => null });
vi.stubGlobal("navigator", { language: "en" });
const { dictionaries } = await import("./i18n");

it("defines the same keys in English and Spanish", () => {
  expect(Object.keys(dictionaries.es).sort()).toEqual(Object.keys(dictionaries.en).sort());
});
