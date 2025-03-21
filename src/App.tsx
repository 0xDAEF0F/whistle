import { useState } from "react";
import Shortcuts from "./Shortcuts";
import MessageDisplay from "./MessageDisplay";
import "./App.css";
import { getShortcuts } from "./utils/shortcuts";

function ShortcutInput() {
  const [pressedKeys, setPressedKeys] = useState<Set<string>>(new Set());
  const [savedKeys, setSavedKeys] = useState<Set<string>>(new Set());
  const [isSettingShortcut, setIsSettingShortcut] = useState(false);

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
        onChange={(e) => {
          console.log(e.target.value);
        }}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
      />
      <div>
        Keys pressed: {pressedKeys.size} ({text})
      </div>
      <button
        onClick={() => {
          console.log(pressedKeys);
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
    console.log(shortcuts);
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
