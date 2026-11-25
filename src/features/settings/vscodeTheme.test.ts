import { describe, expect, it } from "vitest";
import { fromVsCode, parseJsonc } from "./vscodeTheme";

describe("parseJsonc", () => {
  it("drops comments and trailing commas but keeps slashes inside strings", () => {
    const text = `{
      // a line comment
      "url": "https://example.com/a//b", /* block */
      "list": [1, 2,],
    }`;
    expect(parseJsonc(text)).toEqual({ url: "https://example.com/a//b", list: [1, 2] });
  });
});

describe("fromVsCode", () => {
  it("maps editor, accent and terminal colours", () => {
    const theme = fromVsCode(`{
      "name": "Night Owl",
      "type": "dark",
      "colors": {
        "editor.background": "#011627",
        "sideBar.background": "#011627",
        "button.background": "#7e57c2",
        "terminal.ansiRed": "#ef5350",
        "terminal.ansiBrightBlue": "#82aaff",
      },
    }`);
    expect(theme.name).toBe("Night Owl");
    expect(theme.dark).toBe(true);
    expect(theme.ui.canvas).toBe("#011627");
    expect(theme.ui.primary).toBe("#7e57c2");
    expect(theme.terminal.background).toBe("#011627");
    expect(theme.terminal.red).toBe("#ef5350");
    expect(theme.terminal.brightBlue).toBe("#82aaff");
  });

  it("starts light themes from the light preset", () => {
    const theme = fromVsCode(`{ "type": "light", "colors": {} }`);
    expect(theme.dark).toBe(false);
    expect(theme.ui.canvas).toBe("#f6f6f7");
  });

  it("tells light from dark by the background when the file doesn't say", () => {
    expect(fromVsCode(`{ "colors": { "editor.background": "#fdf6e3" } }`).dark).toBe(false);
    expect(fromVsCode(`{ "colors": { "editor.background": "#002b36" } }`).dark).toBe(true);
  });

  it("ignores anything that is not a hex colour", () => {
    const theme = fromVsCode(`{ "colors": { "button.background": "red; background: url(x)" } }`);
    expect(theme.ui.primary).toBe("oklch(0.571 0.21 264)");
  });

  it("explains what is wrong with bad input", () => {
    expect(() => fromVsCode("not json")).toThrow(/not a valid VS Code theme/);
    expect(() => fromVsCode("{}")).toThrow(/no "colors"/);
  });
});
