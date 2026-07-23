import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ProjectDetailDto,
  RunSummaryDto,
  TaskSummaryDto,
} from "@batch-code-analyzer/ipc-types";

const mocks = vi.hoisted(() => ({
  cancelRun: vi.fn(),
  checkBackendHealth: vi.fn(),
  getContext: vi.fn(),
  getProject: vi.fn(),
  getTask: vi.fn(),
  listApiProfiles: vi.fn(),
  listFiles: vi.fn(),
  listProjects: vi.fn(),
  listRuns: vi.fn(),
  listTasks: vi.fn(),
  retryTask: vi.fn(),
  retryTasks: vi.fn(),
  subscribeScanProgress: vi.fn(),
}));

vi.mock("../ipc/health", () => ({
  checkBackendHealth: mocks.checkBackendHealth,
}));
vi.mock("../ipc/projects", () => ({
  addProject: vi.fn(),
  chooseProjectDirectory: vi.fn(),
  getProject: mocks.getProject,
  listProjects: mocks.listProjects,
  saveProjectPrompt: vi.fn(),
  selectProjectPrompt: vi.fn(),
  updateProjectRunSettings: vi.fn(),
}));
vi.mock("../ipc/files", () => ({
  authorizeSensitiveFile: vi.fn(),
  listFiles: mocks.listFiles,
  setFileIncluded: vi.fn(),
}));
vi.mock("../ipc/context", () => ({
  generateContext: vi.fn(),
  getContext: mocks.getContext,
}));
vi.mock("../ipc/scan", () => ({
  cancelScan: vi.fn(),
  getScanReport: vi.fn(),
  startScan: vi.fn(),
  subscribeScanProgress: mocks.subscribeScanProgress,
}));
vi.mock("../ipc/apiProfiles", () => ({
  deleteApiProfile: vi.fn(),
  fetchApiModels: vi.fn(),
  getApiProfileSecret: vi.fn(),
  listApiProfiles: mocks.listApiProfiles,
  putApiProfileSecret: vi.fn(),
  saveApiProfile: vi.fn(),
  testApiProfile: vi.fn(),
}));
vi.mock("../ipc/runs", () => ({
  cancelRun: mocks.cancelRun,
  createRun: vi.fn(),
  executeRun: vi.fn(),
  getTask: mocks.getTask,
  listRuns: mocks.listRuns,
  listTasks: mocks.listTasks,
  previewRun: vi.fn(),
  readResult: vi.fn(),
  retryTask: mocks.retryTask,
  retryTasks: mocks.retryTasks,
}));
vi.mock("../ipc/prompt", () => ({
  generatePrompt: vi.fn(),
}));

import { App } from "./App";

describe("App retry flow", () => {
  afterEach(() => {
    vi.resetAllMocks();
  });

  it("submits individually clicked retries in FIFO order", async () => {
    const user = userEvent.setup();
    const firstRetry = deferred();
    arrangeRetryableRun();
    mocks.retryTask
      .mockReturnValueOnce(firstRetry.promise)
      .mockResolvedValueOnce({});
    render(<App />);

    await user.click(
      await screen.findByRole("button", { name: "重试 src/first.ts" }),
    );
    const secondButton = await screen.findByRole("button", {
      name: "重试 src/second.ts",
    });
    await waitFor(() => expect(secondButton).toBeEnabled());
    await user.click(secondButton);

    expect(mocks.retryTask).toHaveBeenCalledTimes(1);
    expect(screen.getByText("已排队")).toBeInTheDocument();

    await act(async () => {
      firstRetry.resolve({});
    });
    await waitFor(() => expect(mocks.retryTask).toHaveBeenCalledTimes(2));
    expect(mocks.retryTask.mock.calls[0]?.[0]).toEqual({
      projectId: "project-1",
      taskId: "task-1",
    });
    expect(mocks.retryTask.mock.calls[1]?.[0]).toEqual({
      projectId: "project-1",
      taskId: "task-2",
    });
  });

  it("clears pending individual retries when the active Run is cancelled", async () => {
    const user = userEvent.setup();
    const firstRetry = deferred();
    arrangeRetryableRun();
    mocks.retryTask.mockReturnValueOnce(firstRetry.promise);
    mocks.cancelRun.mockResolvedValue({});
    render(<App />);

    await user.click(
      await screen.findByRole("button", { name: "重试 src/first.ts" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "重试 src/second.ts" }),
    );
    await user.click(await screen.findByRole("button", { name: "取消 Run" }));
    await waitFor(() => expect(mocks.cancelRun).toHaveBeenCalledWith("run-1"));

    await act(async () => {
      firstRetry.resolve({});
    });
    await waitFor(() =>
      expect(screen.getByText("没有活动 Run")).toBeInTheDocument(),
    );
    expect(mocks.retryTask).toHaveBeenCalledTimes(1);
  });
});

function arrangeRetryableRun() {
  const run = runSummary();
  const tasks = taskSummaries();
  mocks.checkBackendHealth.mockResolvedValue({
    appVersion: "0.1.0",
    databaseSchemaVersion: 1,
    databaseStatus: "ready",
    schemaVersion: 1,
    status: "ready",
  });
  mocks.listProjects.mockResolvedValue([
    {
      id: "project-1",
      lastOpenedAt: "2026-07-23T10:00:00Z",
      name: "Demo",
      pathStatus: "available",
      schemaVersion: 1,
    },
  ]);
  mocks.getProject.mockResolvedValue(projectDetail());
  mocks.listFiles.mockResolvedValue({ items: [], nextCursor: null, total: 0 });
  mocks.getContext.mockResolvedValue({ context: null });
  mocks.listApiProfiles.mockResolvedValue({
    items: [],
    nextCursor: null,
    total: 0,
  });
  mocks.subscribeScanProgress.mockResolvedValue(() => undefined);
  mocks.listRuns.mockResolvedValue({
    items: [run],
    nextCursor: null,
    total: 1,
  });
  mocks.listTasks.mockResolvedValue({
    items: tasks,
    nextCursor: null,
    total: tasks.length,
  });
  mocks.getTask.mockImplementation(({ taskId }: { taskId: string }) =>
    Promise.resolve({
      attempts: [],
      promptSnapshot: "prompt",
      task: tasks.find((task) => task.id === taskId) ?? tasks[0],
    }),
  );
}

function projectDetail(): ProjectDetailDto {
  return {
    activePromptId: null,
    apiRouting: { fallbacks: [], primaryProfileId: "profile-1" },
    concurrency: 3,
    contextModel: null,
    defaultModel: "gpt-5",
    defaultPrompt: "prompt",
    id: "project-1",
    lastOpenedAt: "2026-07-23T10:00:00Z",
    name: "Demo",
    outputRoot: null,
    pathStatus: "available",
    promptPresets: [],
    schemaVersion: 1,
    sourceDirectory: "C:/workspace/demo",
  };
}

function runSummary(): RunSummaryDto {
  return {
    completedAt: "2026-07-23T10:02:00Z",
    contextVersionId: null,
    createdAt: "2026-07-23T10:00:00Z",
    id: "run-1",
    projectId: "project-1",
    schemaVersion: 1,
    startedAt: "2026-07-23T10:00:01Z",
    stats: {
      cancelled: 0,
      failed: 2,
      interrupted: 0,
      pending: 0,
      queued: 0,
      running: 0,
      sourceChanged: 0,
      succeeded: 0,
      total: 2,
    },
    status: "completed_with_errors",
  };
}

function taskSummaries(): TaskSummaryDto[] {
  return [
    failedTask("task-1", "file-1", "src/first.ts", "attempt-1"),
    failedTask("task-2", "file-2", "src/second.ts", "attempt-2"),
  ];
}

function failedTask(
  id: string,
  fileId: string,
  relativePath: string,
  attemptId: string,
): TaskSummaryDto {
  return {
    completedAt: "2026-07-23T10:02:00Z",
    createdAt: "2026-07-23T10:00:01Z",
    fileId,
    hasResult: false,
    id,
    latestAttemptId: attemptId,
    modelSnapshot: "gpt-5",
    modelSource: "project",
    promptSource: "project",
    relativePath,
    resultVersion: 0,
    runId: "run-1",
    schemaVersion: 1,
    startedAt: "2026-07-23T10:00:02Z",
    status: "failed",
  };
}

function deferred() {
  let resolve!: (value: unknown) => void;
  const promise = new Promise<unknown>((promiseResolve) => {
    resolve = promiseResolve;
  });
  return { promise, resolve };
}
