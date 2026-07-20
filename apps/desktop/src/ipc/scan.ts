import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ProjectId,
  ScanCancelRequest,
  ScanCancelResponse,
  ScanReportDto,
  ScanStartRequest,
  ScanStartResponse,
} from "@batch-code-analyzer/ipc-types";

export function startScan(
  projectId: ProjectId,
  temporaryExcludedPatterns: readonly string[] = [],
): Promise<ScanStartResponse> {
  const request: ScanStartRequest = {
    projectId,
    temporaryExcludedPatterns: [...temporaryExcludedPatterns],
  };
  return invoke<ScanStartResponse>("scan_start", { request });
}

export function cancelScan(operationId: string): Promise<ScanCancelResponse> {
  const request: ScanCancelRequest = { operationId };
  return invoke<ScanCancelResponse>("scan_cancel", { request });
}

export function getScanReport(operationId: string): Promise<ScanReportDto> {
  return invoke<ScanReportDto>("scan_get_report", { operationId });
}

export async function subscribeScanProgress(
  onReport: (report: ScanReportDto) => void,
): Promise<() => void> {
  try {
    return await listen<ScanReportDto>("scan://progress", (event) => {
      onReport(event.payload);
    });
  } catch {
    // Browser previews and older cores do not have the Tauri event bridge.
    return () => undefined;
  }
}
