import { invoke } from "@tauri-apps/api/core";

const HEALTHY_RESPONSE = "ok";

export async function checkBackendHealth(): Promise<void> {
  const response = await invoke<unknown>("health_check");

  if (response !== HEALTHY_RESPONSE) {
    throw new Error("Unexpected health check response");
  }
}
