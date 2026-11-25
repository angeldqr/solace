import { useEffect, useRef, useState } from "react";
import { usePanel } from "./panel";

/**
 * Loads what a view shows, and loads it again whenever files change on disk. At most one load
 * runs per view: changes during a load (a build writing files) queue a single reload instead of
 * piling up git processes. The queued reload always runs the newest request, so switching folder
 * or space mid-load never leaves a view empty.
 */
export function useOnDisk<T>(load: () => Promise<T>, key: string) {
  const version = usePanel((s) => s.version);
  const [result, setResult] = useState<{ data?: T; error?: string }>({});
  const running = useRef(false);
  const queued = useRef<(() => void) | null>(null);
  const latest = useRef(load);
  latest.current = load;

  // biome-ignore lint/correctness/useExhaustiveDependencies: `key` identifies what is loaded and `version` marks changes on disk; `load` is read through a ref.
  useEffect(() => {
    let live = true;
    const run = () => {
      if (running.current) {
        queued.current = run;
        return;
      }
      running.current = true;
      latest
        .current()
        .then(
          (data) => live && setResult({ data }),
          (error) => live && setResult({ error: String(error) }),
        )
        .finally(() => {
          running.current = false;
          const next = queued.current;
          queued.current = null;
          next?.();
        });
    };
    run();
    return () => {
      live = false;
    };
  }, [key, version]);

  return result;
}
