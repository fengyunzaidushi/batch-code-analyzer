import { invoke } from "@tauri-apps/api/core";
import type {
  PromptGenerateRequest,
  PromptGenerateResponse,
} from "@batch-code-analyzer/ipc-types";

export function generatePrompt(
  request: PromptGenerateRequest,
): Promise<PromptGenerateResponse> {
  return invoke<PromptGenerateResponse>("prompt_generate", { request });
}
