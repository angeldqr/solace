import { useRef, useState } from "react";
import type { SpaceId } from "../../shared/ipc";
import { FileTree } from "./FileTree";
import { FileViewer } from "./FileViewer";
import type { Changes } from "./useChanges";

/** The tree stays mounted while a file is open, so expanded folders and scroll position survive. */
export function FilesTab({ space, changes }: { space: SpaceId; changes: Changes }) {
  const [viewing, setViewing] = useState<string | null>(null);
  const tree = useRef<HTMLDivElement>(null);
  const back = () => {
    setViewing(null);
    // The wrapper is still hidden until React commits; focus after that.
    requestAnimationFrame(() =>
      tree.current?.querySelector<HTMLElement>('[role=treeitem][tabindex="0"]')?.focus(),
    );
  };
  return (
    <>
      <div ref={tree} hidden={viewing !== null} className="h-full overflow-auto">
        <FileTree space={space} changes={changes} onOpen={setViewing} />
      </div>
      {viewing !== null && <FileViewer space={space} path={viewing} onBack={back} />}
    </>
  );
}
