import { readTextFile, BaseDirectory } from "@tauri-apps/plugin-fs";

export async function getShortcuts(): Promise<Record<string, string>> {
  let file = await readTextFile(".config/whistle/shortcuts.json", {
    baseDir: BaseDirectory.Home,
  });

  let json = JSON.parse(file);

  return json;
}
