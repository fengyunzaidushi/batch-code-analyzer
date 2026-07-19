import { invoke } from "@tauri-apps/api/core";
import type {
  ApiModelsFetchRequest,
  ApiModelsFetchResponse,
  ApiProfileDeleteRequest,
  ApiProfileDeleteResponse,
  ApiProfileId,
  ApiProfileListResponse,
  ApiProfileSaveRequest,
  ApiProfileSaveResponse,
  ApiProfileTestRequest,
  ApiProfileTestResponse,
} from "@batch-code-analyzer/ipc-types";

export interface ApiProfileSecretPutRequest {
  profileId: ApiProfileId;
  secret: string;
}

export function listApiProfiles(): Promise<ApiProfileListResponse> {
  return invoke<ApiProfileListResponse>("api_profile_list");
}

export function saveApiProfile(
  request: ApiProfileSaveRequest,
): Promise<ApiProfileSaveResponse> {
  return invoke<ApiProfileSaveResponse>("api_profile_save", { request });
}

export function putApiProfileSecret(
  request: ApiProfileSecretPutRequest,
): Promise<ApiProfileSaveResponse> {
  return invoke<ApiProfileSaveResponse>("api_profile_secret_put", { request });
}

export function testApiProfile(
  request: ApiProfileTestRequest,
): Promise<ApiProfileTestResponse> {
  return invoke<ApiProfileTestResponse>("api_profile_test", { request });
}

export function fetchApiModels(
  request: ApiModelsFetchRequest,
): Promise<ApiModelsFetchResponse> {
  return invoke<ApiModelsFetchResponse>("api_models_fetch", { request });
}

export function deleteApiProfile(
  request: ApiProfileDeleteRequest,
): Promise<ApiProfileDeleteResponse> {
  return invoke<ApiProfileDeleteResponse>("api_profile_delete", { request });
}
