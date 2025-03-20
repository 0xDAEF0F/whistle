import { useState } from "react";
import {
  register as registerShortcut,
  unregister as unregisterShortcut,
} from "@tauri-apps/plugin-global-shortcut";

interface ShortcutsProps {
  onMessage: (message: string) => void;
}

export default function Shortcuts({ onMessage }: ShortcutsProps) {
  const [shortcuts, setShortcuts] = useState<string[]>([]);
  const [shortcut, setShortcut] = useState<string>("CmdOrControl+X");

  const register = () => {
    const shortcut_ = shortcut;
    registerShortcut(shortcut_, (e: any) => {
      onMessage(`Shortcut ${shortcut_} triggered ${e.state}`);
    })
      .then(() => {
        setShortcuts((prev) => [...prev, shortcut_]);
        onMessage(`Shortcut ${shortcut_} registered successfully`);
      })
      .catch((err) => onMessage(err.toString()));
  };

  const unregister = (shortcutToUnregister: string) => {
    const shortcut_ = shortcutToUnregister;
    unregisterShortcut(shortcut_)
      .then(() => {
        setShortcuts((prev) => prev.filter((s) => s !== shortcut_));
        onMessage(`Shortcut ${shortcut_} unregistered`);
      })
      .catch((err) => onMessage(err.toString()));
  };

  const unregisterAll = () => {
    unregisterShortcut(shortcuts)
      .then(() => {
        setShortcuts([]);
        onMessage(`Unregistered all shortcuts`);
      })
      .catch((err) => onMessage(err.toString()));
  };

  return (
    <div>
      <div style={{ display: "flex", gap: "4px" }}>
        <input
          style={{ flexGrow: 1 }}
          placeholder="Type a shortcut with '+' as separator..."
          value={shortcut}
          onChange={(e) => setShortcut(e.target.value)}
        />
        <button type="button" onClick={register}>
          Register
        </button>
      </div>
      <br />
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        {shortcuts.map((savedShortcut) => (
          <div
            key={savedShortcut}
            style={{ display: "flex", justifyContent: "space-between" }}
          >
            <span>{savedShortcut}</span>
            <button type="button" onClick={() => unregister(savedShortcut)}>
              Unregister
            </button>
          </div>
        ))}
        {shortcuts.length > 1 && (
          <>
            <br />
            <button type="button" onClick={unregisterAll}>
              Unregister all
            </button>
          </>
        )}
      </div>
    </div>
  );
}
