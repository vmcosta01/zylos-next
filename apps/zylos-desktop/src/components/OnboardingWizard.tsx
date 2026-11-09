import { Component, createSignal, Show } from "solid-js";
import { isFirstRun, completeOnboarding } from "../ipc";

type Step = "welcome" | "gateway" | "complete";

const OnboardingWizard: Component<{ onComplete: () => void }> = (props) => {
  const [step, setStep] = createSignal<Step>("welcome");
  const [gatewayUrl, setGatewayUrl] = createSignal("http://127.0.0.1:9876");
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  const handleComplete = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await completeOnboarding(gatewayUrl());
      if (result.success) {
        setStep("complete");
        setTimeout(() => props.onComplete(), 1500);
      } else {
        setError(result.error || "Unknown error during setup");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{
      display: "flex",
      "flex-direction": "column",
      "align-items": "center",
      "justify-content": "center",
      height: "100vh",
      background: "#0f172a",
      color: "#e2e8f0",
      "font-family": "system-ui, -apple-system, sans-serif",
      padding: "2rem",
    }}>
      <Show when={step() === "welcome"}>
        <h1 style={{ "font-size": "2rem", "margin-bottom": "1rem" }}>Welcome to Zylos Gateway</h1>
        <p style={{ "max-width": "480px", "text-align": "center", "line-height": "1.6", "margin-bottom": "2rem" }}>
          Zylos is your local AI agent orchestration platform. This quick setup will configure
          the embedded gateway so you can start creating agents immediately.
        </p>
        <button
          onClick={() => setStep("gateway")}
          style={{
            padding: "0.75rem 2rem",
            "font-size": "1rem",
            background: "#3b82f6",
            color: "white",
            border: "none",
            "border-radius": "0.5rem",
            cursor: "pointer",
          }}
        >
          Get Started
        </button>
      </Show>

      <Show when={step() === "gateway"}>
        <h2 style={{ "margin-bottom": "1rem" }}>Gateway Configuration</h2>
        <p style={{ "margin-bottom": "1rem", opacity: 0.8 }}>
          The gateway runs locally by default. Change the URL only if connecting to a remote instance.
        </p>
        <input
          type="text"
          value={gatewayUrl()}
          onInput={(e) => setGatewayUrl(e.currentTarget.value)}
          style={{
            padding: "0.5rem 1rem",
            "font-size": "1rem",
            "border-radius": "0.375rem",
            border: "1px solid #475569",
            background: "#1e293b",
            color: "#e2e8f0",
            width: "400px",
            "margin-bottom": "1rem",
          }}
        />
        <Show when={error()}>
          <p style={{ color: "#f87171", "margin-bottom": "1rem" }}>{error()}</p>
        </Show>
        <button
          onClick={handleComplete}
          disabled={loading()}
          style={{
            padding: "0.75rem 2rem",
            "font-size": "1rem",
            background: loading() ? "#64748b" : "#22c55e",
            color: "white",
            border: "none",
            "border-radius": "0.5rem",
            cursor: loading() ? "not-allowed" : "pointer",
          }}
        >
          {loading() ? "Setting up..." : "Complete Setup"}
        </button>
      </Show>

      <Show when={step() === "complete"}>
        <h2 style={{ color: "#22c55e" }}>✓ Setup Complete!</h2>
        <p style={{ opacity: 0.8 }}>Launching Zylos Gateway...</p>
      </Show>
    </div>
  );
};

export default OnboardingWizard;
