import { invoke } from "@tauri-apps/api/core";
import type {
  PageResponse,
  RunCreateRequest,
  RunCreateResponse,
  RunCancelRequest,
  RunCancelResponse,
  RunExecuteRequest,
  RunExecuteResponse,
  RunGetRequest,
  RunGetResponse,
  RunListRequest,
  RunPreviewRequest,
  RunPreviewResponse,
  RunSummaryDto,
  ResultReadRequest,
  ResultReadResponse,
  TaskGetRequest,
  TaskGetResponse,
  TaskListRequest,
  TaskRequestPreviewRequest,
  TaskRequestPreviewResponse,
  TaskRetryBatchRequest,
  TaskRetryBatchResponse,
  TaskRetryRequest,
  TaskRetryResponse,
  TaskSummaryDto,
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

export function cancelRun(runId: string): Promise<RunCancelResponse> {
  const request: RunCancelRequest = { runId };
  return invoke<RunCancelResponse>("run_cancel", { request });
}

export function createRun(
  request: RunCreateRequest,
): Promise<RunCreateResponse> {
  return invoke<RunCreateResponse>("run_create", { request });
}

export function listRuns(
  request: RunListRequest,
): Promise<PageResponse<RunSummaryDto>> {
  return invoke<PageResponse<RunSummaryDto>>("run_list", { request });
}

export function getRun(request: RunGetRequest): Promise<RunGetResponse> {
  return invoke<RunGetResponse>("run_get", { request });
}

export function listTasks(
  request: TaskListRequest,
): Promise<PageResponse<TaskSummaryDto>> {
  return invoke<PageResponse<TaskSummaryDto>>("task_list", { request });
}

export function getTask(request: TaskGetRequest): Promise<TaskGetResponse> {
  return invoke<TaskGetResponse>("task_get", { request });
}

export function getTaskRequestPreview(
  request: TaskRequestPreviewRequest,
): Promise<TaskRequestPreviewResponse> {
  return invoke<TaskRequestPreviewResponse>("task_request_preview", {
    request,
  });
}

export function retryTask(
  request: TaskRetryRequest,
): Promise<TaskRetryResponse> {
  return invoke<TaskRetryResponse>("task_retry", { request });
}

export function retryTasks(
  request: TaskRetryBatchRequest,
): Promise<TaskRetryBatchResponse> {
  return invoke<TaskRetryBatchResponse>("task_retry_batch", { request });
}

export function readResult(
  request: ResultReadRequest,
): Promise<ResultReadResponse> {
  return invoke<ResultReadResponse>("result_read", { request });
}
