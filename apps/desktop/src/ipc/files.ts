import { invoke } from "@tauri-apps/api/core";
import type {
  FileAuthorizeSensitiveRequest,
  FileAuthorizeSensitiveResponse,
  FileListRequest,
  FileRecordId,
  FileRecordSummaryDto,
  FileSetIncludedRequest,
  FileSetIncludedResponse,
  PageResponse,
  ProjectId,
} from "@batch-code-analyzer/ipc-types";

export function authorizeSensitiveFile(
  projectId: ProjectId,
  fileId: FileRecordId,
): Promise<FileAuthorizeSensitiveResponse> {
  const request: FileAuthorizeSensitiveRequest = {
    confirmed: true,
    fileId,
    projectId,
  };
  return invoke<FileAuthorizeSensitiveResponse>("file_authorize_sensitive", {
    request,
  });
}

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
