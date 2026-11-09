import { Component, createSignal, For } from "solid-js";
import { sendPrompt } from "../ipc";

interface Message {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

const ChatPanel: Component = () => {
  const [messages, setMessages] = createSignal<Message[]>([]);
  const [input, setInput] = createSignal("");
  const [sending, setSending] = createSignal(false);

  const handleSend = async () => {
    const text = input().trim();
    if (!text || sending()) return;

    setSending(true);
    setMessages((prev) => [...prev, { role: "user", content: text, timestamp: Date.now() }]);
    setInput("");

    try {
      const ack = await sendPrompt(text);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `[Run ${ack.run_id}] Processing...`, timestamp: Date.now() },
      ]);
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${e}`, timestamp: Date.now() },
      ]);
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div style={{ display: "flex", "flex-direction": "column", flex: "1", overflow: "hidden" }}>
      <div style={{ flex: "1", overflow: "auto", padding: "16px", display: "flex", "flex-direction": "column", gap: "12px" }}>
        <For each={messages()}>
          {(msg) => (
            <div style={{
              "align-self": msg.role === "user" ? "flex-end" : "flex-start",
              background: msg.role === "user" ? "#3b82f6" : "#334155",
              color: "#fff",
              padding: "10px 14px",
              "border-radius": "12px",
              "max-width": "70%",
              "word-break": "break-word",
              "font-size": "14px",
              "line-height": "1.5",
            }}>
              {msg.content}
            </div>
          )}
        </For>
      </div>
      <div style={{ padding: "12px 16px", "border-top": "1px solid #334155", display: "flex", gap: "8px" }}>
        <input
          type="text"
          value={input()}
          onInput={(e) => setInput(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          disabled={sending()}
          style={{
            flex: "1",
            padding: "10px 14px",
            "border-radius": "8px",
            border: "1px solid #475569",
            background: "#1e293b",
            color: "#e2e8f0",
            outline: "none",
            "font-size": "14px",
          }}
        />
        <button
          onClick={handleSend}
          disabled={sending() || !input().trim()}
          style={{
            padding: "10px 20px",
            "border-radius": "8px",
            border: "none",
            background: sending() ? "#475569" : "#3b82f6",
            color: "#fff",
            cursor: sending() ? "not-allowed" : "pointer",
            "font-weight": "600",
            "font-size": "14px",
          }}
        >
          {sending() ? "..." : "Send"}
        </button>
      </div>
    </div>
  );
};

export default ChatPanel;
