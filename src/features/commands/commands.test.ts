import { describe, expect, it, vi } from "vitest";

// commands.ts pulls in the terminal registry and panel store, which need a browser when they load.
vi.mock("../terminals/registry", () => ({ focusTerminal: () => {} }));
vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
const { matchCommand } = await import("./commands");

describe("matchCommand", () => {
  it("matches tokens in any order", () => {
    expect(matchCommand("right split", "Split right", "Terminals")).toBe(true);
  });
  it("ignores case and accents", () => {
    expect(matchCommand("DISENO", "Mover divisor", "Diseño")).toBe(true);
    expect(matchCommand("dividir", "Dividir a la derecha", "Terminales")).toBe(true);
  });
  it("matches the group name", () => {
    expect(matchCommand("terminals new", "New terminal", "Terminals")).toBe(true);
  });
  it("matches an empty query", () => {
    expect(matchCommand("  ", "Split right", "Terminals")).toBe(true);
  });
  it("fails when any token is missing", () => {
    expect(matchCommand("split banana", "Split right", "Terminals")).toBe(false);
  });
});
