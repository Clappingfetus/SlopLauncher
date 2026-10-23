import type { AccountSource, LibraryFolders } from "./types";

export function folderIdentity(path: string): string {
  return path.replaceAll("/", "\\").replace(/\\+$/, "").toLocaleLowerCase();
}
export function addLibraryFolder(
  config: LibraryFolders,
  source: AccountSource,
  path: string,
): string[] {
  const paths = config[source] ?? [];
  if (!path.trim()) throw new Error("Choose a library folder.");
  if (paths.some((p) => folderIdentity(p) === folderIdentity(path)))
    throw new Error("This folder is already included for this source.");
  if (paths.length >= 64)
    throw new Error("This source already has 64 library folders.");
  return [...paths, path];
}
