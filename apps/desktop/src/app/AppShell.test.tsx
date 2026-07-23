import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  AttemptDto,
  ApiProfileSummaryDto,
  ContextVersionDto,
  FileRecordSummaryDto,
  ProjectSummaryDto,
  ResultReadResponse,
  RunPreviewResponse,
  RunSummaryDto,
  TaskGetResponse,
  TaskSummaryDto,
} from "@batch-code-analyzer/ipc-types";

import { AppShell, type ShellProject } from "./AppShell";
import { MarkdownPreview } from "../features/markdown/MarkdownPreview";
import { sanitizeMarkdown } from "../features/markdown/markdownSanitizer";
import { VirtualTaskTable } from "../features/tasks/VirtualTaskTable";

function project(overrides: Partial<ShellProject> = {}): ShellProject {
  return {
    schemaVersion: 1,
    id: "project-1",
    name: "Analyzer Repo",
    pathStatus: "available",
    lastOpenedAt: "2026-07-18T10:00:00Z",
    rootDirectory: "/workspace/analyzer",
    ...overrides,
  } satisfies ProjectSummaryDto & ShellProject;
}

function fileRecord(
  overrides: Partial<FileRecordSummaryDto> = {},
): FileRecordSummaryDto {
  return {
    exclusionReason: null,
    id: "file-1",
    included: true,
    language: "typescript",
    modifiedAt: "2026-07-18T12:00:00Z",
    projectId: "project-1",
    relativePath: "src/main.ts",
    resultStatus: "none",
    schemaVersion: 1,
    sizeBytes: 42,
    sourceStatus: "normal",
    ...overrides,
  };
}

describe("AppShell", () => {
  it("shows the empty project state and delegates adding a project", async () => {
    const user = userEvent.setup();
    const onAddProject = vi.fn();
    render(<AppShell onAddProject={onAddProject} projects={[]} />);

    expect(screen.getByText("还没有项目")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "添加项目" })[1]!);
    expect(onAddProject).toHaveBeenCalledOnce();
  });

  it("keeps an unavailable project visible and labels its path", () => {
    render(<AppShell projects={[project({ pathStatus: "unavailable" })]} />);

    expect(screen.getAllByText("路径不可用")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "重新定位" })).toBeDisabled();
  });

  it("shows the active run and keeps the two tabs fixed", async () => {
    const user = userEvent.setup();
    render(
      <AppShell
        activeRun={{
          projectId: "project-1",
          projectName: "Analyzer Repo",
          status: "running",
        }}
        healthState="ready"
        projects={[project()]}
      />,
    );

    expect(screen.getByText("活动 Run")).toBeInTheDocument();
    expect(screen.getAllByText("运行中")).toHaveLength(2);
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "提示词",
      "API 配置",
    ]);
    await user.click(screen.getByRole("tab", { name: "API 配置" }));
    expect(screen.getByText("API 配置档案")).toBeInTheDocument();
  });

  it("exposes cancellation for a persisted active run", async () => {
    const user = userEvent.setup();
    const onCancelRun = vi.fn();
    render(
      <AppShell
        activeRun={{
          runId: "run-3",
          projectId: "project-1",
          projectName: "Analyzer Repo",
          status: "running",
        }}
        onCancelRun={onCancelRun}
        projects={[project()]}
      />,
    );

    await user.click(screen.getByRole("button", { name: "取消 Run" }));
    expect(onCancelRun).toHaveBeenCalledOnce();
  });

  it("generates a prompt candidate and applies the edited result", async () => {
    const user = userEvent.setup();
    const onGeneratePrompt = vi
      .fn()
      .mockResolvedValue("请分析模块职责和关键数据流。");
    render(
      <AppShell onGeneratePrompt={onGeneratePrompt} projects={[project()]} />,
    );

    await user.click(screen.getByRole("button", { name: "生成提示词" }));
    await user.type(
      screen.getByLabelText("这次分析希望回答什么问题"),
      "梳理核心模块",
    );
    await user.click(screen.getByRole("button", { name: "生成候选" }));
    expect(onGeneratePrompt).toHaveBeenCalledWith("梳理核心模块");
    expect(
      screen.getByDisplayValue("请分析模块职责和关键数据流。"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "使用此提示词" }));
    expect(
      screen.getByDisplayValue("请分析模块职责和关键数据流。"),
    ).toBeInTheDocument();
  });

  it("saves the current prompt as a named project preset", async () => {
    const user = userEvent.setup();
    const onSaveProjectPrompt = vi.fn().mockResolvedValue(undefined);
    render(
      <AppShell
        onSaveProjectPrompt={onSaveProjectPrompt}
        projects={[project()]}
      />,
    );

    await user.clear(screen.getByLabelText("提示词名称"));
    await user.type(screen.getByLabelText("提示词名称"), "架构说明");
    await user.clear(screen.getByLabelText("项目默认提示词"));
    await user.type(
      screen.getByLabelText("项目默认提示词"),
      "请说明模块边界。",
    );
    const saveButton = screen.getByRole("button", { name: "保存为项目默认" });
    expect(saveButton).toBeEnabled();
    await user.click(saveButton);

    expect(onSaveProjectPrompt).toHaveBeenCalledWith({
      name: "架构说明",
      prompt: "请说明模块边界。",
      projectId: "project-1",
    });
  });

  it("selects a saved prompt preset and fills the editor", async () => {
    const user = userEvent.setup();
    const onSelectProjectPrompt = vi.fn().mockResolvedValue(undefined);
    render(
      <AppShell
        onSelectProjectPrompt={onSelectProjectPrompt}
        projects={[
          project({
            activePromptId: "prompt-1",
            defaultPrompt: "解释模块职责。",
            promptPresets: [
              { id: "prompt-1", name: "职责说明", prompt: "解释模块职责。" },
              { id: "prompt-2", name: "影响分析", prompt: "分析修改影响。" },
            ],
          }),
        ]}
      />,
    );

    await user.selectOptions(
      screen.getByLabelText("选择已保存提示词"),
      "prompt-2",
    );
    expect(onSelectProjectPrompt).toHaveBeenCalledWith({
      projectId: "project-1",
      promptId: "prompt-2",
    });
    expect(screen.getByDisplayValue("分析修改影响。")).toBeInTheDocument();
  });

  it("adds a temporary scan exclusion pattern for the current session", async () => {
    const user = userEvent.setup();
    const onAddTemporaryScanPattern = vi.fn();
    render(
      <AppShell
        onAddTemporaryScanPattern={onAddTemporaryScanPattern}
        projects={[project()]}
      />,
    );

    await user.type(screen.getByLabelText("临时排除模式"), "docs/**");
    await user.click(screen.getByRole("button", { name: "添加临时排除模式" }));

    expect(onAddTemporaryScanPattern).toHaveBeenCalledWith("docs/**");
  });

  it("selects and deselects all safe scanned files", async () => {
    const user = userEvent.setup();
    const onSetFileIncluded = vi.fn().mockResolvedValue(undefined);
    const files = [
      fileRecord({ id: "file-included", included: true }),
      fileRecord({
        id: "file-excluded",
        included: false,
        exclusionReason: "user_excluded",
      }),
      fileRecord({
        id: "file-sensitive",
        included: false,
        exclusionReason: "sensitive",
        relativePath: ".env",
        sourceStatus: "sensitive",
      }),
    ];
    const { rerender } = render(
      <AppShell
        fileRecords={files}
        onSetFileIncluded={onSetFileIncluded}
        projects={[project()]}
      />,
    );

    await user.click(screen.getByRole("button", { name: "全选文件" }));
    expect(onSetFileIncluded).toHaveBeenCalledWith("file-excluded", true);
    expect(onSetFileIncluded).not.toHaveBeenCalledWith("file-sensitive", true);

    rerender(
      <AppShell
        fileRecords={files.map((file) =>
          file.id === "file-excluded" ? { ...file, included: true } : file,
        )}
        onSetFileIncluded={onSetFileIncluded}
        projects={[project()]}
      />,
    );
    await user.click(screen.getByRole("button", { name: "取消全选文件" }));
    expect(onSetFileIncluded).toHaveBeenCalledWith("file-included", false);
    expect(onSetFileIncluded).toHaveBeenCalledWith("file-excluded", false);
    expect(onSetFileIncluded).not.toHaveBeenCalledWith("file-sensitive", false);
  });

  it("reenables the bulk button after an inclusion update fails", async () => {
    const user = userEvent.setup();
    const onSetFileIncluded = vi
      .fn()
      .mockRejectedValue(new Error("update failed"));
    render(
      <AppShell
        fileRecords={[fileRecord({ included: false })]}
        onSetFileIncluded={onSetFileIncluded}
        projects={[project()]}
      />,
    );

    const button = screen.getByRole("button", { name: "全选文件" });
    await user.click(button);
    await waitFor(() => expect(button).toBeEnabled());
  });

  it("renders context sources and delegates local discovery", async () => {
    const user = userEvent.setup();
    const onGenerateContext = vi.fn().mockResolvedValue(undefined);
    const context: ContextVersionDto = {
      createdAt: "2026-07-20T10:00:00Z",
      id: "context-1",
      manuallyEdited: false,
      model: null,
      projectId: "project-1",
      schemaVersion: 1,
      sourceFiles: [
        {
          contentHash: "blake3:readme",
          included: true,
          relativePath: "README.md",
          truncated: false,
        },
      ],
      status: "ready",
      summary: "本地发现 1 个项目上下文文件。",
      summaryHash: "blake3:summary",
    };
    render(
      <AppShell
        contextVersion={context}
        onGenerateContext={onGenerateContext}
        projects={[project()]}
      />,
    );

    expect(screen.getByText("README.md")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "重新发现" }));
    expect(onGenerateContext).toHaveBeenCalledOnce();
  });

  it("renders API profile metadata without exposing a key", async () => {
    const user = userEvent.setup();
    const profile: ApiProfileSummaryDto = {
      baseUrl: "https://example.test/v1",
      defaultModel: "gpt-5",
      hasSecret: true,
      id: "profile-1",
      lastConnectionStatus: "unknown",
      lastErrorCode: null,
      lastTestedAt: null,
      modelCache: [],
      modelCacheUpdatedAt: null,
      name: "Local API",
      protocol: "openai-responses",
      schemaVersion: 1,
    };
    render(<AppShell apiProfiles={[profile]} projects={[project()]} />);

    await user.click(screen.getByRole("tab", { name: "API 配置" }));
    expect(screen.getByLabelText("名称")).toHaveValue("Local API");
    expect(screen.getByText("已配置密钥")).toBeInTheDocument();
    expect(screen.queryByDisplayValue(/sk-|api-key/i)).not.toBeInTheDocument();
  });

  it("reveals a configured key only after the eye button is clicked", async () => {
    const user = userEvent.setup();
    const profile: ApiProfileSummaryDto = {
      baseUrl: "https://example.test/v1",
      defaultModel: "gpt-5",
      hasSecret: true,
      id: "profile-1",
      lastConnectionStatus: "unknown",
      lastErrorCode: null,
      lastTestedAt: null,
      modelCache: [],
      modelCacheUpdatedAt: null,
      name: "Local API",
      protocol: "openai-responses",
      schemaVersion: 1,
    };
    const onGetApiProfileSecret = vi
      .fn()
      .mockResolvedValue("test-only-key-value");
    render(
      <AppShell
        apiProfiles={[profile]}
        onGetApiProfileSecret={onGetApiProfileSecret}
        projects={[project()]}
      />,
    );

    await user.click(screen.getByRole("tab", { name: "API 配置" }));
    const input = screen.getByLabelText("API Key");
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveValue("");
    await user.click(screen.getByRole("button", { name: "显示 API Key" }));
    expect(onGetApiProfileSecret).toHaveBeenCalledWith("profile-1");
    expect(input).toHaveAttribute("type", "text");
    expect(input).toHaveValue("test-only-key-value");

    await user.click(screen.getByRole("button", { name: "隐藏 API Key" }));
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveValue("");
  });

  it("saves the selected project API route and default model", async () => {
    const user = userEvent.setup();
    const profile: ApiProfileSummaryDto = {
      baseUrl: "https://example.test/v1",
      defaultModel: "gpt-5",
      hasSecret: true,
      id: "profile-1",
      lastConnectionStatus: "unknown",
      lastErrorCode: null,
      lastTestedAt: null,
      modelCache: [],
      modelCacheUpdatedAt: null,
      name: "Local API",
      protocol: "openai-responses",
      schemaVersion: 1,
    };
    const onUpdateProjectRunSettings = vi.fn().mockResolvedValue(undefined);
    render(
      <AppShell
        apiProfiles={[profile]}
        onUpdateProjectRunSettings={onUpdateProjectRunSettings}
        projects={[project()]}
      />,
    );

    await user.click(screen.getByRole("tab", { name: "API 配置" }));
    await user.click(screen.getByRole("button", { name: "保存项目运行设置" }));

    expect(onUpdateProjectRunSettings).toHaveBeenCalledWith({
      defaultModel: "gpt-5",
      primaryProfileId: "profile-1",
      projectId: "project-1",
    });
  });

  it("saves profile metadata and writes a key through the dedicated handler", async () => {
    const user = userEvent.setup();
    const profile: ApiProfileSummaryDto = {
      baseUrl: "https://example.test/v1",
      defaultModel: "gpt-5",
      hasSecret: false,
      id: "profile-1",
      lastConnectionStatus: "unknown",
      lastErrorCode: null,
      lastTestedAt: null,
      modelCache: [],
      modelCacheUpdatedAt: null,
      name: "Local API",
      protocol: "openai-responses",
      schemaVersion: 1,
    };
    const onSaveApiProfile = vi.fn().mockResolvedValue(profile);
    const onPutApiProfileSecret = vi.fn().mockResolvedValue({
      ...profile,
      hasSecret: true,
    });
    render(
      <AppShell
        apiProfiles={[profile]}
        onPutApiProfileSecret={onPutApiProfileSecret}
        onSaveApiProfile={onSaveApiProfile}
        projects={[project()]}
      />,
    );

    await user.click(screen.getByRole("tab", { name: "API 配置" }));
    await user.clear(screen.getByLabelText("名称"));
    await user.type(screen.getByLabelText("名称"), "Updated API");
    await user.type(screen.getByLabelText("API Key"), "session-secret");
    await user.click(screen.getByRole("button", { name: "保存配置" }));

    expect(onSaveApiProfile).toHaveBeenCalledWith({
      baseUrl: "https://example.test/v1",
      defaultModel: "gpt-5",
      id: "profile-1",
      name: "Updated API",
    });
    expect(onPutApiProfileSecret).toHaveBeenCalledWith({
      profileId: "profile-1",
      secret: "session-secret",
    });
  });

  it("shows a run preview and delegates creation only when it is unblocked", async () => {
    const user = userEvent.setup();
    const onCreateRun = vi.fn();
    const preview: RunPreviewResponse = {
      blockers: [],
      model: "gpt-5",
      modelSource: "project",
      outputDirectory: "/workspace/results",
      projectId: "project-1",
      promptSource: "project",
      schemaVersion: 1,
      tasks: [
        {
          contentHash: "blake3:file",
          fileId: "file-1",
          relativePath: "src/main.rs",
          sizeBytes: 10n,
        },
      ],
    };
    render(
      <AppShell
        onCreateRun={onCreateRun}
        onCloseRunPreview={() => undefined}
        projects={[project()]}
        runPreview={preview}
      />,
    );

    expect(
      screen.getByRole("dialog", { name: "确认本次分析" }),
    ).toBeInTheDocument();
    expect(screen.getByText("src/main.rs")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "创建 Run" }));
    expect(onCreateRun).toHaveBeenCalledOnce();
  });

  it("does not render all rows for a 10,000 item task list", () => {
    const items = Array.from({ length: 10_000 }, (_, index) => `file-${index}`);
    render(
      <VirtualTaskTable
        getRowKey={(item) => item}
        header={<span>文件</span>}
        items={items}
        renderRow={(item) => <span>{item}</span>}
      />,
    );

    expect(screen.getAllByRole("row").length).toBeLessThan(30);
  });

  it("renders Run tasks, Attempt metadata, and delegates result preview", async () => {
    const user = userEvent.setup();
    const onLoadTaskDetail = vi.fn().mockResolvedValue(undefined);
    const onOpenResult = vi.fn().mockResolvedValue(undefined);
    const run = runSummary();
    const task = taskSummary();
    const attempt = attemptDto();
    const detail: TaskGetResponse = {
      attempts: [attempt],
      promptSnapshot: "请解释这个文件的职责。",
      task,
    };
    render(
      <AppShell
        onLoadTaskDetail={onLoadTaskDetail}
        onOpenResult={onOpenResult}
        projects={[project()]}
        runHistory={[run]}
        runTasks={[task]}
        selectedRunId={run.id}
        selectedTaskId={task.id}
        taskDetails={{ [task.id]: detail }}
      />,
    );

    expect(screen.getAllByText("src/main.ts")).not.toHaveLength(0);
    expect(screen.getByText("1 次尝试")).toBeInTheDocument();
    expect(screen.getByText("Local API")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "查看提示词 src/main.ts" }),
    );
    expect(
      screen.getByRole("dialog", {
        name: "发送给 AI 的提示词：src/main.ts",
      }),
    ).toHaveTextContent("请解释这个文件的职责。");
    await user.click(screen.getByRole("button", { name: "关闭提示词预览" }));
    await user.click(screen.getByRole("button", { name: "查看结果" }));
    await user.click(screen.getByRole("button", { name: "1 次尝试" }));
    expect(onOpenResult).toHaveBeenCalledWith(task.id);
    expect(onLoadTaskDetail).toHaveBeenCalledWith(task.id);
  });

  it("offers retry for failed tasks and prevents duplicate submission", async () => {
    const user = userEvent.setup();
    const onRetryTask = vi.fn().mockResolvedValue(undefined);
    const run = {
      ...runSummary(),
      status: "completed_with_errors" as const,
      stats: {
        ...runSummary().stats,
        failed: 1,
        succeeded: 0,
      },
    };
    const task = {
      ...taskSummary(),
      hasResult: false,
      status: "failed" as const,
    };
    const view = render(
      <AppShell
        onRetryTask={onRetryTask}
        projects={[project()]}
        runHistory={[run]}
        runTasks={[task]}
        selectedRunId={run.id}
      />,
    );

    await user.click(screen.getByRole("button", { name: "重试 src/main.ts" }));
    expect(onRetryTask).toHaveBeenCalledWith(task.id);

    view.rerender(
      <AppShell
        onRetryTask={onRetryTask}
        projects={[project()]}
        retryingTaskId={task.id}
        runHistory={[run]}
        runTasks={[task]}
        selectedRunId={run.id}
      />,
    );
    expect(
      screen.getByRole("button", { name: "重试 src/main.ts" }),
    ).toBeDisabled();
    expect(screen.getByText("重试中")).toBeInTheDocument();
  });

  it("shows a sanitized Markdown result dialog when a result is loaded", () => {
    const result: ResultReadResponse = {
      markdown: "# Result\n<script>bad()</script>",
      projectId: "project-1",
      relativePath: "src/main.ts.md",
      resultVersion: 1,
      runId: "run-1",
      schemaVersion: 1,
      taskId: "task-1",
    };
    render(
      <AppShell
        onCloseResultPreview={() => undefined}
        projects={[project()]}
        resultPreview={result}
      />,
    );

    const dialog = screen.getByRole("dialog", { name: "src/main.ts.md" });
    expect(dialog).toBeInTheDocument();
    expect(dialog.querySelector("pre")).toHaveTextContent("# Result");
    expect(dialog.querySelector("pre")).toHaveTextContent("bad()");
    expect(screen.queryByText("<script>")).not.toBeInTheDocument();
  });
});

function runSummary(): RunSummaryDto {
  return {
    completedAt: "2026-07-20T10:02:00Z",
    contextVersionId: null,
    createdAt: "2026-07-20T10:00:00Z",
    id: "run-1",
    projectId: "project-1",
    schemaVersion: 1,
    startedAt: "2026-07-20T10:00:01Z",
    stats: {
      cancelled: 0,
      failed: 0,
      interrupted: 0,
      pending: 0,
      queued: 0,
      running: 0,
      sourceChanged: 0,
      succeeded: 1,
      total: 1,
    },
    status: "completed",
  };
}

function taskSummary(): TaskSummaryDto {
  return {
    completedAt: "2026-07-20T10:02:00Z",
    createdAt: "2026-07-20T10:00:01Z",
    fileId: "file-1",
    hasResult: true,
    id: "task-1",
    latestAttemptId: "attempt-1",
    modelSnapshot: "gpt-5",
    modelSource: "project",
    promptSource: "project",
    relativePath: "src/main.ts",
    resultVersion: 1,
    runId: "run-1",
    schemaVersion: 1,
    startedAt: "2026-07-20T10:00:02Z",
    status: "succeeded",
  };
}

function attemptDto(): AttemptDto {
  return {
    actualModel: "gpt-5",
    apiProfileId: "profile-1",
    apiProfileName: "Local API",
    durationMs: 1200,
    error: null,
    finishedAt: "2026-07-20T10:02:00Z",
    httpStatus: 200,
    id: "attempt-1",
    inputTokens: 10,
    outputTokens: 20,
    retryReason: null,
    schemaVersion: 1,
    sequence: 1,
    startedAt: "2026-07-20T10:00:02Z",
    status: "succeeded",
    taskId: "task-1",
    totalTokens: 30,
  };
}

describe("MarkdownPreview", () => {
  it("removes raw HTML and remote or dangerous destinations", () => {
    const cleaned = sanitizeMarkdown(
      "<script>alert(1)</script> [safe](https://example.test) ![x](https://image.test/x.png) [bad](javascript:alert(1))",
    );
    expect(cleaned).not.toContain("script");
    expect(cleaned).not.toContain("https://");
    expect(cleaned).toContain("safe");
    expect(cleaned).toContain("图片已隐藏");

    render(
      <MarkdownPreview
        content={"# Result"}
        onClose={() => undefined}
        open
        title="结果"
      />,
    );
    expect(screen.getByRole("dialog", { name: "结果" })).toBeInTheDocument();
  });
});
