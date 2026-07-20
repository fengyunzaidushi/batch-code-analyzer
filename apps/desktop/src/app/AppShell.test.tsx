import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  AttemptDto,
  ApiProfileSummaryDto,
  ContextVersionDto,
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
    const detail: TaskGetResponse = { attempts: [attempt], task };
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
    await user.click(screen.getByRole("button", { name: "查看结果" }));
    await user.click(screen.getByRole("button", { name: "1 次尝试" }));
    expect(onOpenResult).toHaveBeenCalledWith(task.id);
    expect(onLoadTaskDetail).toHaveBeenCalledWith(task.id);
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
