import { invoke } from "@tauri-apps/api/core";

export interface UpdateCheckResult {
  available: boolean;
  version?: string;
  body?: string | null;
  date?: string | null;
}

export async function checkForDuskUpdate(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>("check_github_update");
}

export async function installDuskUpdate(): Promise<void> {
  await invoke<void>("install_github_update");
}
