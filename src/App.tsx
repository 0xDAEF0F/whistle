import { useState } from "react";
import Shortcuts from "./Shortcuts";
import MessageDisplay from "./MessageDisplay";
import { getShortcuts } from "./utils/shortcuts";
import "./App.css";

function ShortcutInput() {
  const [pressedKeys, setPressedKeys] = useState<Set<string>>(new Set());
  const [savedKeys, setSavedKeys] = useState<Set<string>>(new Set());
  const [isSettingShortcut, setIsSettingShortcut] = useState(false);

  const [selectedShortcut, setSelectedShortcut] = useState<string>("");

  // console.log({ selectedShortcut });

  const handleKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();

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
          console.log(`selectedShortcut: ${selectedShortcut}`);
          console.log(`savedKeys: ${Array.from(savedKeys)}`);
        }}
      >
        Register
      </button>
    </>
  );
}

function App() {
  const [messages, setMessages] = useState<string[]>([]);

  getShortcuts().then((shortcuts) => {
    // console.log(shortcuts);
  });

  const handleMessage = (message: string) => {
    setMessages((prev) => [message, ...prev].slice(0, 50)); // Keep last 50 messages
  };

  return (
    <div style={{ padding: "0px 20px", maxWidth: "800px", margin: "0 auto" }}>
      <h1>Shortcuts</h1>
      <div style={{ marginBottom: "20px" }}>
        <p>
          Register global shortcuts that will work even when the app is in the
          background. Try using combinations like{" "}
          <code>CommandOrControl+Shift+K</code>.
        </p>
      </div>

      <div
        style={{
          border: "1px solid #ddd",
          padding: "20px",
          borderRadius: "8px",
        }}
      >
        <h2>Manage Shortcuts</h2>
        <Shortcuts onMessage={handleMessage} />
      </div>
      <ShortcutInput />
      <MessageDisplay messages={messages} />
    </div>
  );
}

export default App;
