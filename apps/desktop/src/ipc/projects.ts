import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ProjectSummaryDto } from "@batch-code-analyzer/ipc-types";
import type {
  ProjectAddRequest,
  ProjectAddResponse,
  ProjectDetailDto,
  ProjectId,
  ProjectRunSettingsUpdateRequest,
  ProjectRunSettingsUpdateResponse,
} from "@batch-code-analyzer/ipc-types";

/** Reads project summaries from Rust; no filesystem access happens in React. */
export function listProjects(): Promise<ProjectSummaryDto[]> {
  return invoke<ProjectSummaryDto[]>("project_list");
}

export async function chooseProjectDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择要分析的代码仓库",
  });
  return typeof selected === "string" ? selected : null;
}

export function addProject(
  request: ProjectAddRequest,
): Promise<ProjectAddResponse> {
  return invoke<ProjectAddResponse>("project_add", { request });
}

export function getProject(projectId: ProjectId): Promise<ProjectDetailDto> {
  return invoke<ProjectDetailDto>("project_get", { projectId });
}

export function updateProjectRunSettings(
  request: ProjectRunSettingsUpdateRequest,
): Promise<ProjectRunSettingsUpdateResponse> {
  return invoke<ProjectRunSettingsUpdateResponse>(
    "project_update_run_settings",
    { request },
  );
}
