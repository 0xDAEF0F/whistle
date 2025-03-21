import { useEffect, useState } from "react";
import MessageDisplay from "./components/MessageDisplay";
import { getShortcuts } from "./utils/shortcuts";
import { ShortcutInput } from "./components/ShortcutInput";
import "./App.css";

function App() {
  const [messages, setMessages] = useState<string[]>([]);
  const [shortcuts, setShortcuts] = useState<Record<string, string>>({});

  // Initialization. Load shortcuts from disk and display them.
  useEffect(() => {
    (async () => {
      const shortcuts = await getShortcuts();
      setShortcuts(shortcuts);
    })();
  }, []);

  const handleMessage = (message: string) => {
    setMessages((prev) => [message, ...prev].slice(0, 10));
  };

  return (
    <div style={{ padding: "0px 20px", maxWidth: "800px", margin: "0 auto" }}>
      <h1>Shortcuts</h1>
      <div style={{ marginBottom: "20px" }}>
        <p>
          Register global shortcuts that will work even when the app is in the
          background.
        </p>
      </div>

      <div
        style={{
          border: "1px solid #ddd",
          padding: "20px",
          borderRadius: "8px",
        }}
      >
        <h2>Current Shortcuts</h2>
        {Object.entries(shortcuts).map(([key, value]) => (
          <div key={key}>
            <span>{key}: </span>
            <span>{value}</span>
          </div>
        ))}
      </div>
      <ShortcutInput
        onShortcutRegistered={() => {
          getShortcuts().then((shortcuts) => {
            setShortcuts(shortcuts);
          });
        }}
      />
      <MessageDisplay messages={messages} />
    </div>
  );
}

export default App;
