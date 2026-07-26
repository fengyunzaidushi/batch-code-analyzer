import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScanReportDto } from "@batch-code-analyzer/ipc-types";

const mocks = vi.hoisted(() => ({
  cancelScan: vi.fn(),
  checkBackendHealth: vi.fn(),
  getProject: vi.fn(),
  getScanReport: vi.fn(),
  listFiles: vi.fn(),
  listProjects: vi.fn(),
  setFileIncluded: vi.fn(),
  startScan: vi.fn(),
  subscribeScanProgress: vi.fn(),
}));

vi.mock("../ipc/health", () => ({
  checkBackendHealth: mocks.checkBackendHealth,
}));
vi.mock("../ipc/projects", () => ({
  getProject: mocks.getProject,
  listProjects: mocks.listProjects,
}));
vi.mock("../ipc/files", () => ({
  listFiles: mocks.listFiles,
  setFileIncluded: mocks.setFileIncluded,
}));
vi.mock("../ipc/scan", () => ({
  cancelScan: mocks.cancelScan,
  getScanReport: mocks.getScanReport,
  startScan: mocks.startScan,
  subscribeScanProgress: mocks.subscribeScanProgress,
}));

import { App } from "./App";

describe("App scan flow", () => {
  let onProgress: ((report: ScanReportDto) => void) | undefined;

  afterEach(() => {
    vi.resetAllMocks();
    onProgress = undefined;
  });

  it("starts a scan, renders progress, and cancels the active operation", async () => {
    const user = userEvent.setup();
    const detail = projectDetail();
    const running = scanReport("running");
    arrangeProject(detail);
    mocks.startScan.mockResolvedValue({
      operationId: running.operationId,
      projectId: detail.id,
      schemaVersion: 1,
    });
    mocks.getScanReport.mockResolvedValue(running);
    mocks.cancelScan.mockResolvedValue({
      accepted: true,
      operationId: running.operationId,
      schemaVersion: 1,
    });
    mocks.subscribeScanProgress.mockImplementation(
      (handler: (report: ScanReportDto) => void) => {
        onProgress = handler;
        return Promise.resolve(() => undefined);
      },
    );
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "扫描仓库" }));
    expect(mocks.startScan).toHaveBeenCalledWith(detail.id, []);
    expect(
      await screen.findByRole("button", { name: "取消扫描" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "取消扫描" }));
    expect(mocks.cancelScan).toHaveBeenCalledWith(running.operationId);

    onProgress?.(scanReport("completed"));
    expect(
      await screen.findByText(/扫描完成：纳入 1 个文件/),
    ).toBeInTheDocument();
  });

  it("loads persisted file summaries into the task table", async () => {
    const detail = projectDetail();
    arrangeProject(detail);
    mocks.listFiles.mockResolvedValue({
      items: [fileRecord()],
      nextCursor: null,
      total: 1,
    });
    mocks.subscribeScanProgress.mockResolvedValue(() => undefined);
    render(<App />);

    expect(await screen.findByText("main.rs")).toHaveAttribute(
      "title",
      "src/main.rs",
    );
    expect(screen.getAllByText("待处理")).toHaveLength(2);
  });

  it("persists a file exclusion from the tree", async () => {
    const user = userEvent.setup();
    const detail = projectDetail();
    const record = fileRecord();
    arrangeProject(detail);
    mocks.listFiles.mockResolvedValue({
      items: [record],
      nextCursor: null,
      total: 1,
    });
    mocks.setFileIncluded.mockResolvedValue({
      file: {
        ...record,
        exclusionReason: "user_excluded",
        included: false,
      },
    });
    mocks.subscribeScanProgress.mockResolvedValue(() => undefined);
    render(<App />);

    await user.click(
      await screen.findByRole("checkbox", { name: "排除文件 src/main.rs" }),
    );

    expect(mocks.setFileIncluded).toHaveBeenCalledWith(
      detail.id,
      record.id,
      false,
    );
    expect(await screen.findByText("已排除：用户手动排除")).toBeInTheDocument();
  });
});

function arrangeProject(detail: ReturnType<typeof projectDetail>) {
  mocks.checkBackendHealth.mockResolvedValue({
    appVersion: "0.1.0",
    databaseSchemaVersion: 1,
    databaseStatus: "ready",
    schemaVersion: 1,
    status: "ready",
  });
  mocks.listProjects.mockResolvedValue([
    {
      id: detail.id,
      lastOpenedAt: detail.lastOpenedAt,
      name: detail.name,
      pathStatus: detail.pathStatus,
      schemaVersion: 1,
    },
  ]);
  mocks.getProject.mockResolvedValue(detail);
  mocks.listFiles.mockResolvedValue({ items: [], nextCursor: null, total: 0 });
}

function projectDetail() {
  return {
    apiRouting: { fallbacks: [], primaryProfileId: null },
    contextModel: null,
    defaultModel: null,
    defaultPrompt: "prompt",
    promptPresets: [],
    activePromptId: null,
    id: "project-scan",
    lastOpenedAt: "2026-07-18T12:00:00Z",
    name: "Scan Demo",
    outputRoot: null,
    pathStatus: "available" as const,
    schemaVersion: 1 as const,
    sourceDirectory: "/workspace/scan-demo",
  };
}

function scanReport(status: ScanReportDto["status"]): ScanReportDto {
  return {
    cancelled: status === "cancelled",
    errorCode: null,
    excludedByReason: {},
    fileCount: status === "completed" ? 1 : null,
    generation: status === "completed" ? 1 : null,
    includedFiles: status === "completed" ? 1 : 0,
    invalidGitignoreRules: [],
    operationId: "scan-operation-1",
    projectId: "project-scan",
    rules: {
      builtinDirectories: [],
      builtinExtensions: [],
      gitignoreRules: [],
      sensitiveDetectionEnabled: true,
      temporaryExcludedPatterns: [],
    },
    scannedFiles: status === "running" ? 1 : 2,
    sensitiveFiles: [],
    status,
    schemaVersion: 1,
    symlinkFiles: [],
    unreadableFiles: [],
    unsupportedEncodingFiles: [],
    updatedAt: "2026-07-18T12:00:00Z",
    visitedEntries: 2,
  };
}

function fileRecord() {
  return {
    exclusionReason: null,
    id: "file-main",
    included: true,
    language: "rust",
    modifiedAt: "2026-07-18T12:00:00Z",
    projectId: "project-scan",
    relativePath: "src/main.rs",
    resultStatus: "none" as const,
    schemaVersion: 1 as const,
    sizeBytes: 42,
    sourceStatus: "normal" as const,
  };
}
