import { invoke } from "@tauri-apps/api/core";
import type { ProjectSummaryDto } from "@batch-code-analyzer/ipc-types";

/** Reads project summaries from Rust; no filesystem access happens in React. */
export function listProjects(): Promise<ProjectSummaryDto[]> {
  return invoke<ProjectSummaryDto[]>("project_list");
}
