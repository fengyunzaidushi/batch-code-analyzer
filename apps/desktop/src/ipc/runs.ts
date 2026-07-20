import { invoke } from "@tauri-apps/api/core";
import type {
  RunCreateRequest,
  RunCreateResponse,
  RunExecuteRequest,
  RunExecuteResponse,
  RunPreviewRequest,
  RunPreviewResponse,
} from "@batch-code-analyzer/ipc-types";

export function previewRun(
  request: RunPreviewRequest,
): Promise<RunPreviewResponse> {
  return invoke<RunPreviewResponse>("run_preview", { request });
}

export function executeRun(
  request: RunExecuteRequest,
): Promise<RunExecuteResponse> {
  return invoke<RunExecuteResponse>("run_execute", { request });
}

export function createRun(
  request: RunCreateRequest,
): Promise<RunCreateResponse> {
  return invoke<RunCreateResponse>("run_create", { request });
}
