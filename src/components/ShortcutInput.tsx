import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ShortcutInputProps {
  onShortcutRegistered: () => void;
}

export function ShortcutInput({ onShortcutRegistered }: ShortcutInputProps) {
  const [pressedKeys, setPressedKeys] = useState<Set<string>>(new Set());
  const [savedKeys, setSavedKeys] = useState<Set<string>>(new Set());
  const [isSettingShortcut, setIsSettingShortcut] = useState(false);
  const [selectedShortcut, setSelectedShortcut] = useState<string>("");

  const handleKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();

    // console.log(e.key);

    if (!isSettingShortcut) {
      setIsSettingShortcut(true);
      setPressedKeys(new Set([e.key]));
      setSavedKeys(new Set());
      return;
    }

    setPressedKeys((prev) => {
      const updated = new Set(prev);
      updated.add(e.key);
      return updated;
    });
  };

  const handleKeyUp = (e: React.KeyboardEvent) => {
    if (isSettingShortcut) {
      setSavedKeys(pressedKeys);
    }

    setPressedKeys((prev) => {
      const updated = new Set(prev);
      updated.delete(e.key);
      return updated;
    });

    setIsSettingShortcut(false);
  };

  const text = isSettingShortcut
    ? Array.from(pressedKeys).join(" + ")
    : Array.from(savedKeys).join(" + ");

  return (
    <>
      <input
        type="text"
        placeholder="Press keys to set shortcut"
        value={text}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
      />
      <select
        name="shortcut-type"
        id="shortcut-type"
        value={selectedShortcut}
        onChange={(e) => setSelectedShortcut(e.target.value)}
      >
        <option value="">Select shortcut</option>
        <option value="toggle-recording">Toggle recording</option>
        <option value="cleanse-clipboard">Cleanse clipboard</option>
      </select>
      <button
        onClick={() => {
          invoke("assign_shortcut", {
            name: selectedShortcut,
            shortcut: Array.from(savedKeys)
              .map((key) => {
                if (key.toLowerCase() === "meta") {
                  return "cmd";
                } else if (key.toLowerCase().startsWith("key")) {
                  return key.slice(3);
                } else {
                  return key;
                }
              })
              .join("+"),
          });
          onShortcutRegistered();
        }}
      >
        Register
      </button>
    </>
  );
}
