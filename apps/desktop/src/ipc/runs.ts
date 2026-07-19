import { invoke } from "@tauri-apps/api/core";
import type {
  RunCreateRequest,
  RunCreateResponse,
  RunPreviewRequest,
  RunPreviewResponse,
} from "@batch-code-analyzer/ipc-types";

export function previewRun(
  request: RunPreviewRequest,
): Promise<RunPreviewResponse> {
  return invoke<RunPreviewResponse>("run_preview", { request });
}

export function createRun(
  request: RunCreateRequest,
): Promise<RunCreateResponse> {
  return invoke<RunCreateResponse>("run_create", { request });
}
