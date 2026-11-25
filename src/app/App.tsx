import { Bot, FolderOpen, GitBranch, Monitor, Plus } from "lucide-react";

export function App() {
  return (
    <div className="grid h-full grid-cols-[248px_1fr] grid-rows-[1fr_28px]">
      <aside className="row-span-1 flex flex-col border-line border-r">
        <SidebarSection title="Spaces" action="New space" />
        <p className="px-4 text-2xs text-fg-subtle">No spaces yet</p>
        <div className="mt-auto border-line border-t">
          <SidebarSection title="Agents" />
          <p className="px-4 pb-4 text-2xs text-fg-subtle">Agents you start will appear here</p>
        </div>
      </aside>

      <main className="flex flex-col items-center justify-center gap-5 p-8 text-center">
        <img src="/solace.svg" alt="" className="size-16" />
        <div className="space-y-1.5">
          <h1 className="font-medium text-[17px] text-fg tracking-tight">Start with a folder</h1>
          <p className="max-w-sm text-fg-muted leading-relaxed">
            Each folder becomes a space with its own terminals and agents.
          </p>
        </div>
        <kbd className="flex items-center gap-2 rounded-md border border-line px-2.5 py-1 font-sans text-2xs text-fg-muted">
          <FolderOpen className="size-3.5" aria-hidden />
          Ctrl+O to open a folder
        </kbd>
      </main>

      <footer className="col-span-2 flex items-center gap-4 border-line border-t px-3 text-2xs text-fg-subtle">
        <span className="flex items-center gap-1.5">
          <Monitor className="size-3.5" aria-hidden /> This computer
        </span>
        <span className="flex items-center gap-1.5">
          <GitBranch className="size-3.5" aria-hidden /> No repository
        </span>
        <span className="flex items-center gap-1.5">
          <Bot className="size-3.5" aria-hidden /> 0 agents
        </span>
        <span className="ml-auto">Ctrl+Shift+P commands</span>
      </footer>
    </div>
  );
}

function SidebarSection({ title, action }: { title: string; action?: string }) {
  return (
    <div className="flex h-10 items-center justify-between px-4">
      <h2 className="font-medium text-2xs text-fg-muted">{title}</h2>
      {action && (
        <button
          type="button"
          aria-label={action}
          title={action}
          className="grid size-6 place-items-center rounded-md text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
        >
          <Plus className="size-3.5" aria-hidden />
        </button>
      )}
    </div>
  );
}
