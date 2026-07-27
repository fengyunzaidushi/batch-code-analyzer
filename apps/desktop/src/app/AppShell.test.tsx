import { render, screen, waitFor, within } from "@testing-library/react";
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
  TaskRequestPreviewResponse,
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

  it("keeps an unavailable project visible and delegates relocation", async () => {
    const user = userEvent.setup();
    const onRelocateProject = vi.fn().mockResolvedValue(undefined);
    render(
      <AppShell
        onRelocateProject={onRelocateProject}
        projects={[project({ pathStatus: "unavailable" })]}
      />,
    );

    expect(screen.getAllByText("路径不可用")).toHaveLength(2);
    await user.click(screen.getByRole("button", { name: "重新定位" }));
    expect(onRelocateProject).toHaveBeenCalledOnce();
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

  it("saves the current prompt as a named global prompt", async () => {
    const user = userEvent.setup();
    const onSaveProjectPrompt = vi.fn().mockResolvedValue(undefined);
    render(
      <AppShell
        onSaveProjectPrompt={onSaveProjectPrompt}
        projects={[project()]}
      />,
    );

    await user.clear(screen.getByLabelText("常用提示词名称"));
    await user.type(screen.getByLabelText("常用提示词名称"), "架构说明");
    await user.clear(screen.getByLabelText("项目默认提示词"));
    await user.type(
      screen.getByLabelText("项目默认提示词"),
      "请说明模块边界。",
    );
    const saveButton = screen.getByRole("button", {
      name: "保存为项目默认并加入常用",
    });
    expect(saveButton).toBeEnabled();
    await user.click(saveButton);

    expect(onSaveProjectPrompt).toHaveBeenCalledWith({
      name: "架构说明",
      prompt: "请说明模块边界。",
      projectId: "project-1",
    });
  });

  it("saves edits to the active global prompt", async () => {
    const user = userEvent.setup();
    const onSaveProjectPrompt = vi.fn().mockResolvedValue(undefined);
    render(
      <AppShell
        onSaveProjectPrompt={onSaveProjectPrompt}
        projects={[
          project({
            activePromptId: "prompt-1",
            defaultPrompt: "解释模块职责。",
            promptPresets: [
              { id: "prompt-1", name: "职责说明", prompt: "解释模块职责。" },
            ],
          }),
        ]}
      />,
    );

    const promptEditor = screen.getByLabelText("项目默认提示词");
    await user.clear(promptEditor);
    await user.type(promptEditor, "解释模块职责和边界。");
    await user.click(
      screen.getByRole("button", { name: "保存常用提示词修改" }),
    );

    expect(onSaveProjectPrompt).toHaveBeenCalledWith({
      name: "职责说明",
      prompt: "解释模块职责和边界。",
      projectId: "project-1",
    });
  });

  it("selects a global prompt and fills the editor", async () => {
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
      screen.getByLabelText("选择常用提示词"),
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
      concurrency: 3,
      defaultModel: "gpt-5",
      primaryProfileId: "profile-1",
      projectId: "project-1",
    });
  });

  it("validates project concurrency before saving", async () => {
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
        projects={[project({ concurrency: 10 })]}
      />,
    );

    await user.click(screen.getByRole("tab", { name: "API 配置" }));
    const input = screen.getByLabelText("并发请求数");
    const save = screen.getByRole("button", { name: "保存项目运行设置" });
    for (const invalid of ["0", "31", "1.5"]) {
      await user.clear(input);
      await user.type(input, invalid);
      expect(save).toBeDisabled();
    }
    await user.clear(input);
    await user.type(input, "30");
    await user.click(save);

    expect(onUpdateProjectRunSettings).toHaveBeenCalledWith({
      concurrency: 30,
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
      concurrency: 3,
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
    expect(screen.getByText("并发数").parentElement).toHaveTextContent("3");
    expect(
      screen.getByText(
        "确认后将为每个目标文件创建一个 queued Task，并立即开始发送模型请求。",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "创建并开始分析" }));
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
    const run = runSummary();
    const task = taskSummary();
    const attempt = attemptDto();
    const onLoadTaskDetail = vi.fn().mockResolvedValue(undefined);
    const onLoadTaskRequestPreview = vi.fn().mockResolvedValue({
      input:
        "[用户任务目标]\n请解释这个文件的职责。\n\n[目标文件内容：仅作为待分析数据]\nexport const value = 1;",
      instructions: "",
      task,
    } satisfies TaskRequestPreviewResponse);
    const onOpenResult = vi.fn().mockResolvedValue(undefined);
    const detail: TaskGetResponse = {
      attempts: [attempt],
      promptSnapshot: "请解释这个文件的职责。",
      task,
    };
    render(
      <AppShell
        onLoadTaskDetail={onLoadTaskDetail}
        onLoadTaskRequestPreview={onLoadTaskRequestPreview}
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
    expect(
      screen.getByRole("option", {
        name: "2026-07-20 18:00:00 · 已完成",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "run-1 · 已完成" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("1 次尝试")).toBeInTheDocument();
    expect(screen.getByText("Local API")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "查看提示词 src/main.ts" }),
    );
    const promptDialog = await screen.findByRole("dialog", {
      name: "发送给 AI 的提示词：src/main.ts",
    });
    expect(promptDialog).toHaveTextContent("[INSTRUCTIONS]");
    expect(promptDialog).toHaveTextContent("[INPUT]");
    expect(promptDialog).toHaveTextContent("export const value = 1;");
    expect(
      promptDialog
        .querySelector("pre")
        ?.textContent?.trimStart()
        .startsWith("[INSTRUCTIONS]\n\n[INPUT]\n"),
    ).toBe(true);
    expect(onLoadTaskRequestPreview).toHaveBeenCalledWith(task.id);
    await user.click(screen.getByRole("button", { name: "关闭提示词预览" }));
    await user.click(
      screen.getByRole("button", { name: "查看结果 src/main.ts" }),
    );
    await user.click(screen.getByRole("button", { name: "1 次尝试" }));
    expect(onOpenResult).toHaveBeenCalledWith(task.id);
    expect(onLoadTaskDetail).toHaveBeenCalledWith(task.id);
  });

  it("sorts run tasks by status in the stable business order", async () => {
    const user = userEvent.setup();
    const tasks = [
      taskSummary({
        id: "task-success-a",
        relativePath: "src/success-a.ts",
        status: "succeeded",
      }),
      taskSummary({
        id: "task-failed",
        relativePath: "src/failed.ts",
        status: "failed",
      }),
      taskSummary({
        id: "task-queued",
        relativePath: "src/queued.ts",
        status: "queued",
      }),
      taskSummary({
        id: "task-success-b",
        relativePath: "src/success-b.ts",
        status: "succeeded",
      }),
      taskSummary({
        id: "task-pending",
        relativePath: "src/pending.ts",
        status: "pending",
      }),
      taskSummary({
        id: "task-source-changed",
        relativePath: "src/source-changed.ts",
        status: "source_changed",
      }),
      taskSummary({
        id: "task-running",
        relativePath: "src/running.ts",
        status: "running",
      }),
      taskSummary({
        id: "task-cancelled",
        relativePath: "src/cancelled.ts",
        status: "cancelled",
      }),
      taskSummary({
        id: "task-interrupted",
        relativePath: "src/interrupted.ts",
        status: "interrupted",
      }),
    ];
    const originalOrder = tasks.map((task) => task.relativePath);
    const view = render(
      <AppShell
        projects={[project()]}
        runHistory={[runSummary()]}
        runTasks={tasks}
        selectedRunId="run-1"
      />,
    );

    expect(runTaskPaths()).toEqual(originalOrder);
    await user.click(screen.getByRole("button", { name: "按状态升序排序" }));
    expect(runTaskPaths()).toEqual([
      "src/pending.ts",
      "src/queued.ts",
      "src/running.ts",
      "src/success-a.ts",
      "src/success-b.ts",
      "src/failed.ts",
      "src/cancelled.ts",
      "src/interrupted.ts",
      "src/source-changed.ts",
    ]);
    expect(
      screen.getByRole("button", { name: "按状态降序排序" }).parentElement,
    ).toHaveAttribute("aria-sort", "ascending");

    const refreshedTasks = tasks.map((task) =>
      task.id === "task-failed"
        ? { ...task, status: "pending" as const }
        : task,
    );
    view.rerender(
      <AppShell
        projects={[project()]}
        runHistory={[runSummary()]}
        runTasks={refreshedTasks}
        selectedRunId="run-1"
      />,
    );
    expect(runTaskPaths()).toEqual([
      "src/failed.ts",
      "src/pending.ts",
      "src/queued.ts",
      "src/running.ts",
      "src/success-a.ts",
      "src/success-b.ts",
      "src/cancelled.ts",
      "src/interrupted.ts",
      "src/source-changed.ts",
    ]);

    await user.click(screen.getByRole("button", { name: "按状态降序排序" }));
    expect(runTaskPaths()).toEqual([
      "src/source-changed.ts",
      "src/interrupted.ts",
      "src/cancelled.ts",
      "src/success-a.ts",
      "src/success-b.ts",
      "src/running.ts",
      "src/queued.ts",
      "src/failed.ts",
      "src/pending.ts",
    ]);
    expect(
      screen.getByRole("button", { name: "按状态升序排序" }).parentElement,
    ).toHaveAttribute("aria-sort", "descending");
    expect(tasks.map((task) => task.relativePath)).toEqual(originalOrder);
  });

  it("formats and sorts request times while keeping empty values last", async () => {
    const user = userEvent.setup();
    const earlyTime = new Date(2026, 0, 2, 3, 4, 5).toISOString();
    const lateTime = new Date(2026, 0, 2, 4, 5, 6).toISOString();
    const tasks = [
      taskSummary({
        id: "task-late",
        relativePath: "src/late.ts",
        startedAt: lateTime,
      }),
      taskSummary({
        id: "task-empty",
        relativePath: "src/empty.ts",
        startedAt: null,
      }),
      taskSummary({
        id: "task-early-a",
        relativePath: "src/early-a.ts",
        startedAt: earlyTime,
      }),
      taskSummary({
        id: "task-early-b",
        relativePath: "src/early-b.ts",
        startedAt: earlyTime,
      }),
    ];
    render(
      <AppShell
        projects={[project()]}
        runHistory={[runSummary()]}
        runTasks={tasks}
        selectedRunId="run-1"
      />,
    );

    const table = screen.getByRole("table", { name: "运行结果任务" });
    const columnHeaders = within(table).getAllByRole("columnheader");
    expect(columnHeaders).toHaveLength(6);
    expect(columnHeaders[2]).toHaveTextContent("请求时间");
    for (const row of within(table).getAllByRole("row").slice(1)) {
      expect(within(row).getAllByRole("cell")).toHaveLength(6);
    }
    expect(within(table).getAllByText("2026-01-02 03:04:05")).toHaveLength(2);
    expect(runTaskRequestTime("src/empty.ts")).toBe("—");

    await user.click(
      screen.getByRole("button", { name: "按请求时间升序排序" }),
    );
    expect(runTaskPaths()).toEqual([
      "src/early-a.ts",
      "src/early-b.ts",
      "src/late.ts",
      "src/empty.ts",
    ]);
    expect(
      screen.getByRole("button", { name: "按请求时间降序排序" }).parentElement,
    ).toHaveAttribute("aria-sort", "ascending");
    expect(
      screen.getByRole("button", { name: "按状态升序排序" }).parentElement,
    ).not.toHaveAttribute("aria-sort");

    await user.click(
      screen.getByRole("button", { name: "按请求时间降序排序" }),
    );
    expect(runTaskPaths()).toEqual([
      "src/late.ts",
      "src/early-a.ts",
      "src/early-b.ts",
      "src/empty.ts",
    ]);

    await user.click(screen.getByRole("button", { name: "按状态升序排序" }));
    expect(
      screen.getByRole("button", { name: "按请求时间升序排序" }).parentElement,
    ).not.toHaveAttribute("aria-sort");
  });

  it("keeps request-time sorting across Run changes and preserves Task actions", async () => {
    const user = userEvent.setup();
    const onOpenResult = vi.fn().mockResolvedValue(undefined);
    const onSelectRun = vi.fn();
    const earlyTime = new Date(2026, 1, 3, 4, 5, 6).toISOString();
    const lateTime = new Date(2026, 1, 3, 5, 6, 7).toISOString();
    const runOne = runSummary();
    const runTwo = runSummary({ id: "run-2" });
    const firstTasks = [
      taskSummary({
        id: "task-late",
        relativePath: "src/late.ts",
        startedAt: lateTime,
      }),
      taskSummary({
        id: "task-early",
        relativePath: "src/early.ts",
        startedAt: earlyTime,
      }),
    ];
    const view = render(
      <AppShell
        onOpenResult={onOpenResult}
        onSelectRun={onSelectRun}
        projects={[project()]}
        runHistory={[runOne, runTwo]}
        runTasks={firstTasks}
        selectedRunId={runOne.id}
      />,
    );

    await user.click(
      screen.getByRole("button", { name: "按请求时间升序排序" }),
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "选择 Run" }),
      runTwo.id,
    );
    expect(onSelectRun).toHaveBeenCalledWith(runTwo.id);

    const refreshedTasks = [
      taskSummary({
        id: "task-empty",
        relativePath: "src/empty.ts",
        runId: runTwo.id,
        startedAt: null,
      }),
      taskSummary({
        id: "task-late",
        relativePath: "src/late.ts",
        runId: runTwo.id,
        startedAt: lateTime,
      }),
      taskSummary({
        id: "task-early",
        relativePath: "src/early.ts",
        runId: runTwo.id,
        startedAt: earlyTime,
      }),
    ];
    view.rerender(
      <AppShell
        onOpenResult={onOpenResult}
        onSelectRun={onSelectRun}
        projects={[project()]}
        runHistory={[runOne, runTwo]}
        runTasks={refreshedTasks}
        selectedRunId={runTwo.id}
      />,
    );

    expect(runTaskPaths()).toEqual([
      "src/early.ts",
      "src/late.ts",
      "src/empty.ts",
    ]);
    expect(
      screen.getByRole("button", { name: "按请求时间降序排序" }).parentElement,
    ).toHaveAttribute("aria-sort", "ascending");
    await user.click(
      screen.getByRole("button", { name: "查看结果 src/early.ts" }),
    );
    expect(onOpenResult).toHaveBeenCalledWith("task-early");
  });

  it("disables prompt preview while the complete request is loading", async () => {
    const user = userEvent.setup();
    const run = runSummary();
    const task = taskSummary();
    const onLoadTaskRequestPreview = vi.fn(
      () => new Promise<TaskRequestPreviewResponse>(() => undefined),
    );
    render(
      <AppShell
        onLoadTaskRequestPreview={onLoadTaskRequestPreview}
        projects={[project()]}
        runHistory={[run]}
        runTasks={[task]}
        selectedRunId={run.id}
      />,
    );

    const button = screen.getByRole("button", {
      name: "查看提示词 src/main.ts",
    });
    await user.click(button);
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps other failed tasks clickable while one retry is queued", async () => {
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
    const secondTask = {
      ...task,
      id: "task-2",
      relativePath: "src/worker.ts",
    };
    const view = render(
      <AppShell
        onRetryTask={onRetryTask}
        projects={[project()]}
        runHistory={[run]}
        runTasks={[task, secondTask]}
        selectedRunId={run.id}
      />,
    );

    await user.click(screen.getByRole("button", { name: "重试 src/main.ts" }));
    expect(onRetryTask).toHaveBeenCalledWith(task.id);

    view.rerender(
      <AppShell
        onRetryTask={onRetryTask}
        projects={[project()]}
        retryingTaskIds={[task.id]}
        runHistory={[run]}
        runTasks={[task, secondTask]}
        selectedRunId={run.id}
      />,
    );
    expect(
      screen.getByRole("button", { name: "重试 src/main.ts" }),
    ).toBeDisabled();
    expect(screen.getByText("重试中")).toBeInTheDocument();
    const secondRetry = screen.getByRole("button", {
      name: "重试 src/worker.ts",
    });
    expect(secondRetry).toBeEnabled();
    await user.click(secondRetry);
    expect(onRetryTask).toHaveBeenLastCalledWith(secondTask.id);
  });

  it("opens failed tasks through the result action and shows failure reasons", async () => {
    const user = userEvent.setup();
    const task = {
      ...taskSummary(),
      hasResult: false,
      resultVersion: 0,
      status: "failed" as const,
    };
    const firstAttempt: AttemptDto = {
      ...attemptDto(),
      error: {
        code: "provider_timeout",
        message: "模型请求未完成",
        retryable: true,
        sanitized: true,
      },
      httpStatus: null,
      status: "failed_retryable",
      totalTokens: null,
    };
    const lastAttempt: AttemptDto = {
      ...firstAttempt,
      error: {
        code: "provider_server_error",
        message: "模型请求未完成",
        retryable: false,
        sanitized: true,
      },
      httpStatus: 503,
      id: "attempt-2",
      sequence: 2,
      status: "failed_terminal",
    };
    const detail: TaskGetResponse = {
      attempts: [firstAttempt, lastAttempt],
      promptSnapshot: "请分析",
      task,
    };
    const onOpenResult = vi.fn().mockResolvedValue(undefined);
    const view = render(
      <AppShell
        onOpenResult={onOpenResult}
        projects={[project()]}
        runHistory={[runSummary()]}
        runTasks={[task]}
        selectedRunId="run-1"
      />,
    );

    await user.click(
      screen.getByRole("button", { name: "查看结果 src/main.ts" }),
    );
    expect(onOpenResult).toHaveBeenCalledWith(task.id);

    view.rerender(
      <AppShell
        failurePreview={detail}
        projects={[project()]}
        runHistory={[runSummary()]}
        runTasks={[task]}
        selectedRunId="run-1"
      />,
    );
    const dialog = screen.getByRole("dialog", {
      name: "失败结果：src/main.ts",
    });
    expect(dialog).toHaveTextContent("分析失败");
    expect(dialog).toHaveTextContent("模型服务暂时不可用。");
    expect(dialog).toHaveTextContent("provider_server_error");
    expect(dialog).toHaveTextContent("模型请求超时。");
    expect(dialog).toHaveTextContent("provider_timeout");
    expect(dialog).toHaveTextContent("503");
    expect(dialog).toHaveTextContent("Local API");
    expect(dialog).toHaveTextContent("gpt-5");
  });

  it("does not display an unsanitized failure message", () => {
    const task = {
      ...taskSummary(),
      hasResult: false,
      resultVersion: 0,
      status: "failed" as const,
    };
    const attempt: AttemptDto = {
      ...attemptDto(),
      error: {
        code: "provider_future_error",
        message: "sensitive backend detail",
        retryable: false,
        sanitized: false,
      },
      status: "failed_terminal",
    };
    render(
      <AppShell
        failurePreview={{
          attempts: [attempt],
          promptSnapshot: "请分析",
          task,
        }}
        projects={[project()]}
      />,
    );

    const dialog = screen.getByRole("dialog", {
      name: "失败结果：src/main.ts",
    });
    expect(dialog).toHaveTextContent("请求失败，但没有可安全显示的详细原因。");
    expect(dialog).toHaveTextContent("provider_future_error");
    expect(dialog).not.toHaveTextContent("sensitive backend detail");
  });

  it("submits all failed tasks through the batch retry action", async () => {
    const user = userEvent.setup();
    const onRetryTasks = vi.fn().mockResolvedValue(undefined);
    const run = {
      ...runSummary(),
      status: "completed_with_errors" as const,
      stats: { ...runSummary().stats, failed: 2, succeeded: 0 },
    };
    const firstTask = {
      ...taskSummary(),
      hasResult: false,
      status: "failed" as const,
    };
    const secondTask = {
      ...firstTask,
      id: "task-2",
      relativePath: "src/worker.ts",
    };
    const view = render(
      <AppShell
        onRetryTasks={onRetryTasks}
        projects={[project()]}
        runHistory={[run]}
        runTasks={[firstTask, secondTask]}
        selectedRunId={run.id}
      />,
    );

    await user.click(screen.getByRole("button", { name: "重试全部失败（2）" }));
    expect(onRetryTasks).toHaveBeenCalledWith([firstTask.id, secondTask.id]);

    view.rerender(
      <AppShell
        isBatchRetrying
        onRetryTasks={onRetryTasks}
        projects={[project()]}
        runHistory={[run]}
        runTasks={[firstTask, secondTask]}
        selectedRunId={run.id}
      />,
    );
    expect(
      screen.getByRole("button", { name: "批量重试中（2）" }),
    ).toBeDisabled();
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

function runSummary(overrides: Partial<RunSummaryDto> = {}): RunSummaryDto {
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
    ...overrides,
  };
}

function taskSummary(overrides: Partial<TaskSummaryDto> = {}): TaskSummaryDto {
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
    ...overrides,
  };
}

function runTaskPaths(): string[] {
  const table = screen.getByRole("table", { name: "运行结果任务" });
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell")[0]?.textContent ?? "");
}

function runTaskRequestTime(path: string): string {
  const table = screen.getByRole("table", { name: "运行结果任务" });
  const row = within(table)
    .getAllByRole("row")
    .find((candidate) => candidate.textContent?.includes(path));
  if (!row) throw new Error(`Missing Task row: ${path}`);
  return within(row).getAllByRole("cell")[2]?.textContent ?? "";
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
