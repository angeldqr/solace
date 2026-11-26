import type { MessageKey } from "../../shared/i18n";
import type { Settings } from "./settings";

export type PageId =
  | "general"
  | "appearance"
  | "terminal"
  | "diffs"
  | "agents"
  | "shell"
  | "shortcuts"
  | "about";

/** keywords: the i18n keys of the page's row titles and descriptions, so search finds them. */
export const PAGES: { id: PageId; title: MessageKey; keywords: MessageKey[] }[] = [
  {
    id: "general",
    title: "general",
    keywords: [
      "language",
      "interfaceLanguage",
      "languageHint",
      "notifications",
      "notifyAgents",
      "updates",
      "checkUpdates",
      "privacyNote",
    ],
  },
  {
    id: "appearance",
    title: "appearance",
    keywords: [
      "appearanceMode",
      "appearanceSystem",
      "appearanceLight",
      "appearanceDark",
      "theme",
      "importVsCode",
      "interfaceSection",
      "interfaceSize",
      "interfaceSizeHint",
      "contrast",
      "contrastHint",
      "glass",
      "glassHint",
      "customize",
      "accentColour",
      "accentDefault",
      "typography",
      "uiFont",
      "codeFont",
      "codeFontSize",
      "fontSmoothing",
      "animationSpeed",
      "customCss",
      "customCssHint",
      "editThemeColours",
    ],
  },
  {
    id: "terminal",
    title: "terminalSection",
    keywords: [
      "fontFamily",
      "fontSize",
      "lineHeight",
      "cursorStyle",
      "cursorBlink",
      "terminalSample",
    ],
  },
  {
    id: "diffs",
    title: "diffsPage",
    keywords: ["diffStyle", "diffColours", "wrapLines", "ignoreWhitespace", "ignoreWhitespaceHint"],
  },
  {
    id: "agents",
    title: "agents",
    keywords: ["claudeIntegration", "claudeIntegrationHint"],
  },
  { id: "shell", title: "shell", keywords: ["shellForNewTerminals", "shellDefault"] },
  {
    id: "shortcuts",
    title: "shortcuts",
    keywords: [
      "keyboardShortcuts",
      "commands",
      "openFolder",
      "newTerminal",
      "splitRight",
      "splitDown",
      "closeTerminal",
      "moveDividerLeft",
      "moveDividerRight",
      "moveDividerUp",
      "moveDividerDown",
      "nextTab",
      "previousTab",
      "toggleFiles",
      "toggleChanges",
      "settings",
    ],
  },
  {
    id: "about",
    title: "about",
    keywords: ["version", "license", "privacyNote", "updateReady", "upToDate"],
  },
];

/** The settings each page's "Restore defaults" resets. */
export const RESETTABLE: Partial<Record<PageId, (keyof Settings)[]>> = {
  appearance: [
    "theme",
    "appearance",
    "uiScale",
    "contrast",
    "glassOpacity",
    "accent",
    "uiFont",
    "codeFont",
    "codeFontSize",
    "fontSmoothing",
    "motionMs",
    "customCss",
  ],
  diffs: ["diffLayout", "diffColors", "wordWrap", "diffIgnoreWhitespace"],
  terminal: ["fontFamily", "fontSize", "lineHeight", "cursorStyle", "cursorBlink"],
};

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();

/** The pages whose title or any row matches the query, ignoring case and accents. */
export function filterPages(query: string, translate: (k: MessageKey) => string): PageId[] {
  const needle = fold(query.trim());
  return PAGES.filter(
    (page) =>
      !needle ||
      [page.title, ...page.keywords].some((key) => fold(translate(key)).includes(needle)),
  ).map((page) => page.id);
}
