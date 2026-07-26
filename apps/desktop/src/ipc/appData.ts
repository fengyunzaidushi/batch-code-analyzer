import { invoke } from "@tauri-apps/api/core";
import type {
  AppDataResetRequest,
  AppDataResetResponse,
} from "@batch-code-analyzer/ipc-types";

export function resetAppData(
  request: AppDataResetRequest,
): Promise<AppDataResetResponse> {
  return invoke<AppDataResetResponse>("app_data_reset", { request });
}
