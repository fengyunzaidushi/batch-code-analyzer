import { invoke } from "@tauri-apps/api/core";
import type {
  FileListRequest,
  FileRecordSummaryDto,
  PageResponse,
  ProjectId,
} from "@batch-code-analyzer/ipc-types";

export function listFiles(
  projectId: ProjectId,
  cursor?: string,
  limit = 500,
): Promise<PageResponse<FileRecordSummaryDto>> {
  const request: FileListRequest = {
    projectId,
    limit,
    ...(cursor ? { cursor } : {}),
  };
  return invoke<PageResponse<FileRecordSummaryDto>>("file_list", { request });
}
