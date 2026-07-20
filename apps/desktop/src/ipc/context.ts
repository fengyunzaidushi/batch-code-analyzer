import { invoke } from "@tauri-apps/api/core";
import type {
  ContextGenerateRequest,
  ContextGenerateResponse,
  ContextGetRequest,
  ContextGetResponse,
  ProjectId,
} from "@batch-code-analyzer/ipc-types";

export function generateContext(
  projectId: ProjectId,
): Promise<ContextGenerateResponse> {
  const request: ContextGenerateRequest = { projectId };
  return invoke<ContextGenerateResponse>("context_generate", { request });
}

export function getContext(projectId: ProjectId): Promise<ContextGetResponse> {
  const request: ContextGetRequest = { projectId };
  return invoke<ContextGetResponse>("context_get", { request });
}
