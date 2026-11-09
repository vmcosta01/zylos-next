// IPC bridge — typed wrappers for Tauri invoke calls
// Matches crates/zylos-desktop/src/lib.rs IpcRequest / IpcResponse types

import { invoke } from "@tauri-apps/api/core";

export interface GatewayStatus {
  healthy: boolean;
  version: string;
  uptime_secs: number;
  active_agents: number;
}

export interface SkillSummary {
  name: string;
  description: string;
  category: string | null;
}

export interface PlaybookSummary {
  id: string;
  name: string;
  description: string;
  steps: number;
}

export interface ActionResult {
  success: boolean;
  result: unknown | null;
  error: string | null;
}

export interface UpdateInfo {
  version: string;
  changelog: string;
}

export interface IpcError {
  code: string;
  message: string;
}

// --- Commands ---

export async function getStatus(): Promise<GatewayStatus> {
  return invoke("get_status");
}

export async function sendPrompt(message: string, threadId?: string): Promise<{ run_id: string }> {
  return invoke("send_prompt", { message, threadId });
}

export async function listSkills(): Promise<SkillSummary[]> {
  return invoke("list_skills");
}

export async function listPlaybooks(): Promise<PlaybookSummary[]> {
  return invoke("list_playbooks");
}

export async function executeAction(
  provider: string,
  action: string,
  params: Record<string, unknown>,
): Promise<ActionResult> {
  return invoke("execute_action", { provider, action, params });
}

export async function openExternal(url: string): Promise<void> {
  return invoke("open_external", { url });
}

export async function minimizeToTray(): Promise<void> {
  return invoke("minimize_to_tray");
}

export async function checkUpdate(): Promise<UpdateInfo | null> {
  return invoke("check_update");
}

export async function isFirstRun(): Promise<boolean> {
  return invoke("is_first_run");
}

export async function completeOnboarding(
  gatewayUrl?: string,
): Promise<ActionResult> {
  return invoke("complete_onboarding", { gatewayUrl });
}
