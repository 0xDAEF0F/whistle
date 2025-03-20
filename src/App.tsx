import { useState } from "react";
import Shortcuts from "./Shortcuts";
import MessageDisplay from "./MessageDisplay";
import "./App.css";

function App() {
  const [messages, setMessages] = useState<string[]>([]);

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

      <MessageDisplay messages={messages} />
    </div>
  );
}

export default App;
