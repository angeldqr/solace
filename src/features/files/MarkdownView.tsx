import { useMemo, useRef } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { api } from "../../shared/ipc";

const slug = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}_\- ]/gu, "")
    .trim()
    .replace(/ /g, "-");

const textOf = (node: React.ReactNode): string =>
  typeof node === "string"
    ? node
    : Array.isArray(node)
      ? node.map(textOf).join("")
      : node && typeof node === "object" && "props" in node
        ? textOf((node.props as { children?: React.ReactNode }).children)
        : "";

const WEB = /^(https?:\/\/|mailto:)/i;

function heading(Tag: "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
  return ({ children }: { children?: React.ReactNode }) => (
    <Tag id={slug(textOf(children))}>{children}</Tag>
  );
}

/** Loaded lazily: the Markdown parser stays out of the main chunk until a .md file opens. */
export default function MarkdownView({ text }: { text: string }) {
  const scroller = useRef<HTMLDivElement>(null);
  const components = useMemo<React.ComponentProps<typeof Markdown>["components"]>(
    () => ({
      h1: heading("h1"),
      h2: heading("h2"),
      h3: heading("h3"),
      h4: heading("h4"),
      h5: heading("h5"),
      h6: heading("h6"),
      a({ href, children }) {
        if (href?.startsWith("#")) {
          return (
            <a
              href={href}
              onClick={(event) => {
                event.preventDefault();
                // A malformed escape like "%of" would throw; fall back to the raw text.
                let id = href.slice(1);
                try {
                  id = decodeURIComponent(id);
                } catch {}
                scroller.current
                  ?.querySelector(`[id="${CSS.escape(id)}"]`)
                  ?.scrollIntoView({ block: "start" });
              }}
            >
              {children}
            </a>
          );
        }
        if (href && WEB.test(href)) {
          return (
            <button
              type="button"
              title={href}
              onClick={() => void api.openUrl(href).catch(() => {})}
              className="md-link"
            >
              {children}
            </button>
          );
        }
        // Relative links have nowhere to go inside the viewer.
        return <span title={href}>{children}</span>;
      },
      // shortcut: images render as their alt text; the CSP blocks remote ones and relative ones are not resolved. Resolve them through file_read if needed.
      img: ({ alt }) => <span className="text-fg-subtle italic">{alt}</span>,
    }),
    [],
  );
  return (
    <div ref={scroller} className="min-h-0 flex-1 overflow-auto">
      <div className="md-body max-w-[72ch] select-text px-5 py-4">
        <Markdown remarkPlugins={[remarkGfm]} skipHtml components={components}>
          {text}
        </Markdown>
      </div>
    </div>
  );
}
