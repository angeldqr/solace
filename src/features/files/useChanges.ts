import { api, type Change, type SpaceId } from "../../shared/ipc";
import { useOnDisk } from "./useOnDisk";

export const STATUS_COLOR: Record<string, string> = {
  M: "text-warning",
  A: "text-success",
  U: "text-success",
  D: "text-danger",
  R: "text-info",
  "!": "text-danger",
  C: "text-info",
};

/** What git reports for a space: undefined while loading, null when it is not a repository. */
export type Changes = { data?: Change[] | null; error?: string };

export function useChanges(space: SpaceId): Changes {
  return useOnDisk(() => api.gitStatus(space), String(space));
}

/** Git paths are relative to the space root; tolerate backslashes so they match the tree's. */
export const slash = (path: string) => path.replace(/\\/g, "/");
