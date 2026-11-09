import { Component, createSignal, onMount, Show } from "solid-js";
import StatusBar from "./components/StatusBar";
import ChatPanel from "./components/ChatPanel";
import OnboardingWizard from "./components/OnboardingWizard";
import { isFirstRun } from "./ipc";

const App: Component = () => {
  const [needsOnboarding, setNeedsOnboarding] = createSignal<boolean | null>(null);

  onMount(async () => {
    try {
      const first = await isFirstRun();
      setNeedsOnboarding(first);
    } catch {
      // If IPC fails, skip onboarding and show main UI
      setNeedsOnboarding(false);
    }
  });

  return (
    <div style={{ display: "flex", "flex-direction": "column", height: "100vh" }}>
      <Show when={needsOnboarding() === true}>
        <OnboardingWizard onComplete={() => setNeedsOnboarding(false)} />
      </Show>
      <Show when={needsOnboarding() === false}>
        <StatusBar />
        <ChatPanel />
      </Show>
    </div>
  );
};

export default App;
