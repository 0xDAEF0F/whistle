interface MessageDisplayProps {
  messages: string[];
}

export default function MessageDisplay({ messages }: MessageDisplayProps) {
  return (
    <div
      style={{
        marginTop: "20px",
        padding: "10px",
        border: "1px solid #ccc",
        borderRadius: "4px",
        backgroundColor: "#f5f5f5",
        maxHeight: "200px",
        overflow: "auto",
      }}
    >
      <h3 style={{ margin: "0 0 10px 0" }}>Messages:</h3>
      {messages.length === 0 ? (
        <p>No messages yet</p>
      ) : (
        <ul style={{ margin: 0, padding: "0 0 0 20px" }}>
          {messages.map((message, index) => (
            <li key={index}>{message}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
