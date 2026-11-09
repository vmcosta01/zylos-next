import { Component, createSignal, onMount, Show } from "solid-js";
import { getStatus, type GatewayStatus } from "../ipc";

const StatusBar: Component = () => {
  const [status, setStatus] = createSignal<GatewayStatus | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const refresh = async () => {
    try {
      const s = await getStatus();
      setStatus(s);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  onMount(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  });

  const dotColor = () => {
    const s = status();
    if (!s) return "#6b7280";
    return s.healthy ? "#22c55e" : "#ef4444";
  };

  return (
    <div style={{
      display: "flex",
      "align-items": "center",
      gap: "12px",
      padding: "8px 16px",
      background: "#1e293b",
      "border-bottom": "1px solid #334155",
      "font-size": "13px",
    }}>
      <span style={{
        width: "8px",
        height: "8px",
        "border-radius": "50%",
        background: dotColor(),
        display: "inline-block",
      }} />
      <Show when={status()} fallback={<span style={{ color: "#94a3b8" }}>Connecting...</span>}>
        {(s) => (
          <>
            <span style={{ color: "#e2e8f0", "font-weight": "600" }}>Zylos Gateway</span>
            <span style={{ color: "#94a3b8" }}>v{s().version}</span>
            <span style={{ color: "#94a3b8" }}>|</span>
            <span style={{ color: "#94a3b8" }}>{s().active_agents} agents</span>
            <span style={{ color: "#94a3b8" }}>|</span>
            <span style={{ color: "#94a3b8" }}>uptime {Math.floor(s().uptime_secs / 60)}m</span>
          </>
        )}
      </Show>
      <Show when={error()}>
        <span style={{ color: "#ef4444", "margin-left": "auto" }}>{error()}</span>
      </Show>
    </div>
  );
};

export default StatusBar;
