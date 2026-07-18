import { invoke } from "@tauri-apps/api/core";
import type {
  FileListRequest,
  FileRecordId,
  FileRecordSummaryDto,
  FileSetIncludedRequest,
  FileSetIncludedResponse,
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

export function setFileIncluded(
  projectId: ProjectId,
  fileId: FileRecordId,
  included: boolean,
): Promise<FileSetIncludedResponse> {
  const request: FileSetIncludedRequest = {
    fileId,
    included,
    projectId,
  };
  return invoke<FileSetIncludedResponse>("file_set_included", { request });
}
