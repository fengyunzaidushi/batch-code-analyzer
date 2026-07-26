import { invoke } from "@tauri-apps/api/core";
import type { HealthCheckResponse } from "@batch-code-analyzer/ipc-types";

export function checkBackendHealth(): Promise<HealthCheckResponse> {
  return invoke<HealthCheckResponse>("health_check");
}
