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

  const handleKeyUpDownMsg = (message: string) => {
    setMessages((prev) => [message, ...prev].slice(0, 10));
  };

  return (
    <div className="mt-5 max-w-[800px] mx-auto space-y-3 px-5">
      <h1 className="text-2xl font-bold text-center">Shortcuts</h1>
      <div className="mb-5">
        <p>
          Register global shortcuts that will work even when the app is in the
          background.
        </p>
      </div>

      <div
        className="space-y-1"
        style={{
          border: "1px solid #ddd",
          padding: "20px",
          borderRadius: "8px",
        }}
      >
        <h2 className="text-md font-bold">
          Current Shortcuts{" "}
          <span className="font-semibold text-sm">
            (Override shortcuts by setting a new shortcut in the input below)
          </span>
        </h2>
        <ul className="list-disc list-inside">
          {Object.entries(shortcuts).map(([key, value]) => (
            <li className="flex items-center gap-x-2">
              <span className="">- {key}: </span>
              <span>{value}</span>
            </li>
          ))}
        </ul>
      </div>
      <ShortcutInput
        handleKeyUpDownMsg={handleKeyUpDownMsg}
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
