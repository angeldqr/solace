import { describe, expect, it } from "vitest";
import type { Layout } from "./ipc";
import { splitAround } from "./session";

const leaf = (pane: number): Layout => ({ type: "leaf", pane });

describe("splitAround", () => {
  // 1 | (2 over 3)
  const layout: Layout = {
    type: "split",
    direction: "row",
    ratio: 0.5,
    first: leaf(1),
    second: { type: "split", direction: "column", ratio: 0.3, first: leaf(2), second: leaf(3) },
  };

  it("finds the closest split in the asked direction", () => {
    expect(splitAround(layout, 3, "column")).toEqual({ path: [1], ratio: 0.3 });
    expect(splitAround(layout, 3, "row")).toEqual({ path: [], ratio: 0.5 });
  });

  it("returns nothing without such a split or pane", () => {
    expect(splitAround(layout, 1, "column")).toBeUndefined();
    expect(splitAround(layout, 9, "row")).toBeUndefined();
    expect(splitAround(leaf(1), 1, "row")).toBeUndefined();
  });
});
