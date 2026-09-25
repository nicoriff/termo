import { invoke } from "@tauri-apps/api/core";

/** A folder the agent-launch browser can show (see
 * src-tauri/src/commands/folder_commands.rs). */
export interface FolderEntry {
  name: string;
  path: string;
}

export interface FolderRoots {
  /** Drives on Windows; `/` elsewhere. */
  drives: FolderEntry[];
  home: FolderEntry | null;
}

export function listFolderRoots(): Promise<FolderRoots> {
  return invoke("list_folder_roots");
}

/** The visible folders inside `path`, sorted by name. Rejects with a message
 * ready to show when the folder can't be read. */
export function listFolders(path: string): Promise<FolderEntry[]> {
  return invoke("list_folders", { path });
}

/** Creates `name` inside `parent`. Rejects, with a message ready to show,
 * when the name isn't valid or a folder by that name already exists. */
export function createFolder(parent: string, name: string): Promise<FolderEntry> {
  return invoke("create_folder", { parent, name });
}
