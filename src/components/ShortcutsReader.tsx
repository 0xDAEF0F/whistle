import { useState, useEffect } from "react";
import { getShortcuts } from "../utils/shortcuts";

interface Shortcut {
  key: string;
  action: string;
}

export default function ShortcutsReader() {
  const [shortcuts, setShortcuts] = useState<Shortcut[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadShortcuts = async () => {
      try {
        setLoading(true);
        const data = await getShortcuts();
        setShortcuts(data);
        setError(null);
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to load shortcuts"
        );
        console.error("Error loading shortcuts:", err);
      } finally {
        setLoading(false);
      }
    };

    loadShortcuts();
  }, []);

  return (
    <div>
      <h2>Shortcuts</h2>
      {loading && <p>Loading shortcuts...</p>}
      {error && <p style={{ color: "red" }}>Error: {error}</p>}
      {!loading && !error && shortcuts.length === 0 && (
        <p>No shortcuts found in ~/.config/whistle/shortcuts.json</p>
      )}
      {shortcuts.length > 0 && (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th
                style={{
                  textAlign: "left",
                  padding: "8px",
                  borderBottom: "1px solid #ddd",
                }}
              >
                Shortcut
              </th>
              <th
                style={{
                  textAlign: "left",
                  padding: "8px",
                  borderBottom: "1px solid #ddd",
                }}
              >
                Action
              </th>
            </tr>
          </thead>
          <tbody>
            {shortcuts.map((shortcut, index) => (
              <tr key={index}>
                <td style={{ padding: "8px", borderBottom: "1px solid #ddd" }}>
                  {shortcut.key}
                </td>
                <td style={{ padding: "8px", borderBottom: "1px solid #ddd" }}>
                  {shortcut.action}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
