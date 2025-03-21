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
        backgroundColor: "#e8e8e8",
        maxHeight: "200px",
        overflow: "auto",
        color: "black",
      }}
    >
      <h3 className="text-md font-bold">Key presses:</h3>
      {messages.length === 0 ? (
        <p>No key presses yet</p>
      ) : (
        <ul className="list-disc list-inside">
          {messages.map((message, index) => (
            <li key={index}>{message}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
