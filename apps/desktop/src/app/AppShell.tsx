import {
  Activity,
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ChevronRight,
  CircleAlert,
  Eye,
  EyeOff,
  FolderPlus,
  GitBranch,
  LayoutGrid,
  ListChecks,
  ListX,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  Sparkles,
  X,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import type {
  ApiProfileSaveRequest,
  ApiProfileSummaryDto,
  AttemptDto,
  ContextVersionDto,
  FileRecordSummaryDto,
  ProjectPathStatus,
  ProjectPromptSaveRequest,
  ProjectPromptSelectRequest,
  ProjectRunSettingsUpdateRequest,
  PromptPresetDto,
  ProjectSummaryDto,
  ResultReadResponse,
  RunPreviewResponse,
  RunSummaryDto,
  ScanRuleSummaryDto,
  ScanReportDto,
  TaskGetResponse,
  TaskRequestPreviewResponse,
  TaskSummaryDto,
} from "@batch-code-analyzer/ipc-types";

import { FileTreeTable } from "../features/tasks/FileTreeTable";
import { canIncludeFile } from "../features/tasks/fileSelection";
import { VirtualTaskTable } from "../features/tasks/VirtualTaskTable";
import { MarkdownPreview } from "../features/markdown/MarkdownPreview";
import { DataManagementPanel } from "../features/data-management/DataManagementPanel";

export type ShellProject = ProjectSummaryDto & {
  rootDirectory?: string;
  primaryProfileId?: string | null;
  defaultModel?: string | null;
  concurrency?: number;
  defaultPrompt?: string;
  promptPresets?: readonly PromptPresetDto[];
  activePromptId?: string | null;
  runningTaskCount?: number;
  failedTaskCount?: number;
};

export type ShellHealthState =
  "checking" | "ready" | "degraded" | "unavailable";

export interface ActiveRunSummary {
  runId?: string;
  projectId: string;
  projectName: string;
  status: "running" | "paused" | "cancelling" | "interrupted";
}

interface AppShellProps {
  apiProfileError?: string | null;
  apiProfiles?: readonly ApiProfileSummaryDto[];
  fileRecords?: readonly FileRecordSummaryDto[];
  fileTotal?: number;
  projects?: readonly ShellProject[];
  healthState?: ShellHealthState;
  activeRun?: ActiveRunSummary | null;
  onCancelRun?: () => void;
  onAddProject?: () => void;
  onRelocateProject?: () => Promise<void>;
  isRelocatingProject?: boolean;
  onAuthorizeSensitiveFile?: (fileId: string) => Promise<void>;
  onGenerateContext?: () => Promise<void> | void;
  onCancelScan?: () => void;
  onAddTemporaryScanPattern?: (pattern: string) => void;
  onRemoveTemporaryScanPattern?: (pattern: string) => void;
  onRetryHealth?: () => void;
  onSetFileIncluded?: (fileId: string, included: boolean) => Promise<void>;
  onPreviewRun?: (input: { prompt: string }) => void;
  onGeneratePrompt?: (goal: string) => Promise<string>;
  onSaveProjectPrompt?: (request: ProjectPromptSaveRequest) => Promise<void>;
  onSelectProjectPrompt?: (
    request: ProjectPromptSelectRequest,
  ) => Promise<void>;
  onCreateRun?: () => Promise<void> | void;
  onCloseRunPreview?: () => void;
  runPreview?: RunPreviewResponse | null;
  runError?: string | null;
  runHistory?: readonly RunSummaryDto[];
  runResultsError?: string | null;
  runTasks?: readonly TaskSummaryDto[];
  selectedRunId?: string | null;
  selectedTaskId?: string | null;
  taskDetails?: Readonly<Record<string, TaskGetResponse>>;
  isLoadingRunResults?: boolean;
  onLoadTaskDetail?: (taskId: string) => Promise<void>;
  onLoadTaskRequestPreview?: (
    taskId: string,
  ) => Promise<TaskRequestPreviewResponse>;
  onOpenResult?: (taskId: string) => Promise<void>;
  onRetryTask?: (taskId: string) => Promise<void>;
  onRetryTasks?: (taskIds: readonly string[]) => Promise<void>;
  onSelectRun?: (runId: string) => void;
  retryingTaskIds?: readonly string[];
  isBatchRetrying?: boolean;
  batchRetryTargetCount?: number;
  resultPreview?: ResultReadResponse | null;
  failurePreview?: TaskGetResponse | null;
  onCloseResultPreview?: () => void;
  isCreatingRun?: boolean;
  onDeleteApiProfile?: (id: string) => Promise<void>;
  onFetchApiModels?: (id: string) => Promise<void>;
  onPutApiProfileSecret?: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onGetApiProfileSecret?: (profileId: string) => Promise<string>;
  onSaveApiProfile?: (
    request: ApiProfileSaveRequest,
  ) => Promise<ApiProfileSummaryDto>;
  onTestApiProfile?: (id: string) => Promise<void>;
  onUpdateProjectRunSettings?: (
    request: ProjectRunSettingsUpdateRequest,
  ) => Promise<void>;
  onSelectProject?: (id: string) => void;
  onResetAppData?: () => Promise<void>;
  onStartScan?: () => void;
  projectError?: string | null;
  scanReport?: ScanReportDto | null;
  selectedProjectId?: string | null;
  isAddingProject?: boolean;
  isGeneratingContext?: boolean;
  contextVersion?: ContextVersionDto | null;
  temporaryScanPatterns?: readonly string[];
}

type WorkspaceTab = "prompt" | "api";

const DEFAULT_PROMPT =
  "请结合提供的项目上下文，用通俗但准确的语言解释当前代码文件。\n\n请说明核心职责、关键输入输出、协作模块和修改影响。";
const BULK_FILE_UPDATE_BATCH_SIZE = 16;

export function AppShell({
  apiProfileError = null,
  apiProfiles = [],
  fileRecords = [],
  fileTotal = 0,
  projects = [],
  healthState = "checking",
  activeRun = null,
  onCancelRun = () => undefined,
  onAddProject = () => undefined,
  onRelocateProject = async () => undefined,
  isRelocatingProject = false,
  onAuthorizeSensitiveFile = async () => undefined,
  onGenerateContext = async () => undefined,
  onCancelScan = () => undefined,
  onAddTemporaryScanPattern = () => undefined,
  onRemoveTemporaryScanPattern = () => undefined,
  onRetryHealth = () => undefined,
  onSetFileIncluded = async () => undefined,
  onPreviewRun = () => undefined,
  onGeneratePrompt = async () => "",
  onSaveProjectPrompt = async () => undefined,
  onSelectProjectPrompt = async () => undefined,
  onCreateRun = async () => undefined,
  onCloseRunPreview = () => undefined,
  runPreview = null,
  runError = null,
  runHistory = [],
  runResultsError = null,
  runTasks = [],
  selectedRunId = null,
  selectedTaskId = null,
  taskDetails = {},
  isLoadingRunResults = false,
  onLoadTaskDetail = async () => undefined,
  onLoadTaskRequestPreview = async () => {
    throw new Error("请求预览处理器不可用");
  },
  onOpenResult = async () => undefined,
  onRetryTask = async () => undefined,
  onRetryTasks = async () => undefined,
  onSelectRun = () => undefined,
  retryingTaskIds = [],
  isBatchRetrying = false,
  batchRetryTargetCount = 0,
  resultPreview = null,
  failurePreview = null,
  onCloseResultPreview = () => undefined,
  isCreatingRun = false,
  onDeleteApiProfile = async () => undefined,
  onFetchApiModels = async () => undefined,
  onPutApiProfileSecret = async () => {
    throw new Error("API Profile secret handler is unavailable");
  },
  onGetApiProfileSecret = async () => {
    throw new Error("API Profile secret reveal handler is unavailable");
  },
  onSaveApiProfile = async () => {
    throw new Error("API Profile save handler is unavailable");
  },
  onTestApiProfile = async () => undefined,
  onUpdateProjectRunSettings = async () => undefined,
  onSelectProject,
  onResetAppData = async () => undefined,
  onStartScan = () => undefined,
  projectError = null,
  scanReport = null,
  selectedProjectId: controlledSelectedProjectId,
  isAddingProject = false,
  isGeneratingContext = false,
  contextVersion = null,
  temporaryScanPatterns = [],
}: AppShellProps) {
  const [internalSelectedProjectId, setInternalSelectedProjectId] = useState<
    string | null
  >(projects[0]?.id ?? null);
  const [search, setSearch] = useState("");
  const [tab, setTab] = useState<WorkspaceTab>("prompt");
  const [dataManagementOpen, setDataManagementOpen] = useState(false);
  const [promptDraft, setPromptDraft] = useState({
    projectId: null as string | null,
    value: DEFAULT_PROMPT,
  });
  const selectedProjectId =
    controlledSelectedProjectId === undefined
      ? (internalSelectedProjectId ?? projects[0]?.id ?? null)
      : controlledSelectedProjectId;

  const selectProject = (id: string) => {
    setInternalSelectedProjectId(id);
    onSelectProject?.(id);
  };

  const filteredProjects = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    if (!query) return projects;
    return projects.filter((project) =>
      `${project.name} ${project.rootDirectory ?? ""}`
        .toLocaleLowerCase()
        .includes(query),
    );
  }, [projects, search]);
  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? null;

  const prompt =
    promptDraft.projectId === selectedProjectId
      ? promptDraft.value
      : (selectedProject?.defaultPrompt ?? DEFAULT_PROMPT);
  const setPrompt = (value: string) => {
    setPromptDraft({ projectId: selectedProjectId, value });
  };

  return (
    <div className="desktop-shell">
      <GlobalRunBar
        activeRun={activeRun}
        healthState={healthState}
        onCancelRun={onCancelRun}
        onRetryHealth={onRetryHealth}
      />
      <div className="shell-body">
        <ProjectSidebar
          activeRun={activeRun}
          onAddProject={onAddProject}
          onSelect={selectProject}
          onOpenDataManagement={() => setDataManagementOpen(true)}
          projects={filteredProjects}
          projectError={projectError}
          search={search}
          selectedProjectId={selectedProjectId}
          setSearch={setSearch}
          isAddingProject={isAddingProject}
        />
        <ProjectWorkspace
          activeRun={activeRun}
          fileRecords={fileRecords}
          fileTotal={fileTotal}
          contextVersion={contextVersion}
          onAuthorizeSensitiveFile={onAuthorizeSensitiveFile}
          onRelocateProject={onRelocateProject}
          isRelocatingProject={isRelocatingProject}
          onGenerateContext={onGenerateContext}
          isGeneratingContext={isGeneratingContext}
          onCancelScan={onCancelScan}
          onAddTemporaryScanPattern={onAddTemporaryScanPattern}
          onRemoveTemporaryScanPattern={onRemoveTemporaryScanPattern}
          onSetFileIncluded={onSetFileIncluded}
          onPreviewRun={() => onPreviewRun({ prompt })}
          onGeneratePrompt={onGeneratePrompt}
          onSaveProjectPrompt={onSaveProjectPrompt}
          onSelectProjectPrompt={onSelectProjectPrompt}
          onPromptChange={setPrompt}
          prompt={prompt}
          onStartScan={onStartScan}
          project={selectedProject}
          scanReport={scanReport}
          temporaryScanPatterns={temporaryScanPatterns}
          tab={tab}
          setTab={setTab}
          apiProfileError={apiProfileError}
          apiProfiles={apiProfiles}
          onDeleteApiProfile={onDeleteApiProfile}
          onFetchApiModels={onFetchApiModels}
          onGetApiProfileSecret={onGetApiProfileSecret}
          onPutApiProfileSecret={onPutApiProfileSecret}
          onSaveApiProfile={onSaveApiProfile}
          onTestApiProfile={onTestApiProfile}
          onUpdateProjectRunSettings={onUpdateProjectRunSettings}
          runHistory={runHistory}
          runResultsError={runResultsError}
          runTasks={runTasks}
          selectedRunId={selectedRunId}
          selectedTaskId={selectedTaskId}
          taskDetails={taskDetails}
          isLoadingRunResults={isLoadingRunResults}
          onLoadTaskDetail={onLoadTaskDetail}
          onLoadTaskRequestPreview={onLoadTaskRequestPreview}
          onOpenResult={onOpenResult}
          onRetryTask={onRetryTask}
          onRetryTasks={onRetryTasks}
          onSelectRun={onSelectRun}
          retryBlocked={
            activeRun !== null &&
            (activeRun.status !== "running" ||
              isBatchRetrying ||
              retryingTaskIds.length === 0 ||
              activeRun.runId !== selectedRunId)
          }
          retryingTaskIds={retryingTaskIds}
          isBatchRetrying={isBatchRetrying}
          batchRetryTargetCount={batchRetryTargetCount}
        />
      </div>
      <RunPreviewPanel
        error={runError}
        isCreating={isCreatingRun}
        onClose={onCloseRunPreview}
        onCreate={() => void onCreateRun()}
        preview={runPreview}
      />
      {runError && !runPreview ? (
        <div className="run-error-toast" role="alert">
          {runError}
        </div>
      ) : null}
      <MarkdownPreview
        content={resultPreview?.markdown ?? ""}
        onClose={onCloseResultPreview}
        open={resultPreview !== null}
        title={resultPreview?.relativePath ?? "分析结果"}
      />
      {failurePreview ? (
        <FailureResultPreview
          detail={failurePreview}
          onClose={onCloseResultPreview}
        />
      ) : null}
      <DataManagementPanel
        active={dataManagementOpen}
        onClose={() => setDataManagementOpen(false)}
        onResetAppData={onResetAppData}
      />
    </div>
  );
}

function GlobalRunBar({
  activeRun,
  healthState,
  onCancelRun,
  onRetryHealth,
}: {
  activeRun: ActiveRunSummary | null;
  healthState: ShellHealthState;
  onCancelRun: () => void;
  onRetryHealth: () => void;
}) {
  return (
    <header className="global-run-bar">
      <div className="brand-lockup">
        <span aria-hidden="true" className="brand-symbol">
          <Activity size={17} strokeWidth={2.4} />
        </span>
        <div>
          <strong>Batch Code Analyzer</strong>
          <span>LOCAL WORKSPACE</span>
        </div>
      </div>
      <div className="global-statuses">
        <HealthStatus onRetry={onRetryHealth} state={healthState} />
        {activeRun ? (
          <div className="run-status" role="status">
            <span className="run-dot" />
            <span>{activeRun.projectName}</span>
            <span className="muted-status">
              {runStatusLabel(activeRun.status)}
            </span>
            {activeRun.runId ? (
              <button
                aria-label="取消 Run"
                className="run-cancel-button"
                disabled={activeRun.status === "cancelling"}
                onClick={onCancelRun}
                title="取消 Run"
                type="button"
              >
                <X aria-hidden="true" size={13} />
                取消
              </button>
            ) : null}
          </div>
        ) : (
          <span className="muted-status">没有活动 Run</span>
        )}
      </div>
    </header>
  );
}

function HealthStatus({
  onRetry,
  state,
}: {
  onRetry: () => void;
  state: ShellHealthState;
}) {
  const labels: Record<ShellHealthState, string> = {
    checking: "正在检查",
    ready: "本地核心已就绪",
    degraded: "本地核心降级",
    unavailable: "无法连接本地核心",
  };
  return (
    <>
      <span className={`core-status core-status-${state}`} role="status">
        <span aria-hidden="true" className="core-status-dot" />
        {labels[state]}
      </span>
      {state === "degraded" || state === "unavailable" ? (
        <button className="health-retry" onClick={onRetry} type="button">
          <RefreshCw aria-hidden="true" size={13} />
          重新检查
        </button>
      ) : null}
    </>
  );
}

function ProjectSidebar({
  activeRun,
  onAddProject,
  onSelect,
  onOpenDataManagement,
  projects,
  projectError,
  search,
  selectedProjectId,
  setSearch,
  isAddingProject,
}: {
  activeRun: ActiveRunSummary | null;
  onAddProject: () => void;
  onSelect: (id: string) => void;
  onOpenDataManagement: () => void;
  projects: readonly ShellProject[];
  projectError: string | null;
  search: string;
  selectedProjectId: string | null;
  setSearch: (value: string) => void;
  isAddingProject: boolean;
}) {
  return (
    <aside aria-label="项目侧栏" className="project-sidebar">
      <div className="sidebar-heading">
        <div>
          <p className="eyebrow">PROJECTS</p>
          <h2>项目</h2>
        </div>
        <button
          aria-label="添加项目"
          className="icon-button sidebar-add"
          onClick={onAddProject}
          title="添加项目"
          type="button"
        >
          <Plus aria-hidden="true" size={18} />
        </button>
      </div>
      <label className="search-field">
        <Search aria-hidden="true" size={15} />
        <span className="sr-only">搜索项目</span>
        <input
          onChange={(event) => setSearch(event.target.value)}
          placeholder="搜索项目"
          type="search"
          value={search}
        />
      </label>
      {projectError ? (
        <div className="project-error" role="alert">
          {projectError}
        </div>
      ) : null}
      <div className="project-list" role="list">
        {projects.length === 0 ? (
          <EmptyProjectState
            isAddingProject={isAddingProject}
            onAddProject={onAddProject}
          />
        ) : (
          projects.map((project) => (
            <ProjectListItem
              activeRun={activeRun}
              isSelected={project.id === selectedProjectId}
              key={project.id}
              onSelect={() => onSelect(project.id)}
              project={project}
            />
          ))
        )}
      </div>
      <div className="sidebar-footer">
        <button
          className="sidebar-footer-action"
          onClick={onOpenDataManagement}
          type="button"
        >
          <Settings2 aria-hidden="true" size={16} />
          <span>工作区设置</span>
        </button>
      </div>
    </aside>
  );
}

function EmptyProjectState({
  isAddingProject,
  onAddProject,
}: {
  isAddingProject: boolean;
  onAddProject: () => void;
}) {
  return (
    <div className="empty-project-state">
      <div className="empty-state-icon" aria-hidden="true">
        <FolderPlus size={20} />
      </div>
      <strong>还没有项目</strong>
      <p>添加一个本地代码仓库开始分析。</p>
      <button
        className="secondary-button"
        disabled={isAddingProject}
        onClick={onAddProject}
        type="button"
      >
        <Plus aria-hidden="true" size={15} />
        {isAddingProject ? "正在选择" : "添加项目"}
      </button>
    </div>
  );
}

function ProjectListItem({
  activeRun,
  isSelected,
  onSelect,
  project,
}: {
  activeRun: ActiveRunSummary | null;
  isSelected: boolean;
  onSelect: () => void;
  project: ShellProject;
}) {
  const isRunning = activeRun?.projectId === project.id;
  return (
    <button
      className={`project-list-item${isSelected ? " is-selected" : ""}`}
      onClick={onSelect}
      role="listitem"
      type="button"
    >
      <span className="project-list-icon" aria-hidden="true">
        <GitBranch size={16} />
      </span>
      <span className="project-list-copy">
        <strong>{project.name}</strong>
        <span>{project.rootDirectory ?? "路径由 Rust 管理"}</span>
        <span className="project-list-meta">
          <PathStatus status={project.pathStatus} />
          {isRunning ? <em>运行中</em> : null}
          {project.failedTaskCount ? (
            <em className="error-meta">{project.failedTaskCount} 失败</em>
          ) : null}
        </span>
      </span>
      <ChevronRight
        aria-hidden="true"
        className="project-list-arrow"
        size={15}
      />
    </button>
  );
}

function PathStatus({ status }: { status: ProjectPathStatus }) {
  return (
    <span className={`path-status path-status-${status}`}>
      <span aria-hidden="true" />
      {status === "available" ? "可用" : "路径不可用"}
    </span>
  );
}

function ProjectWorkspace({
  apiProfileError,
  apiProfiles,
  activeRun,
  fileRecords,
  fileTotal,
  contextVersion,
  onAuthorizeSensitiveFile,
  onRelocateProject,
  isRelocatingProject,
  onGenerateContext,
  isGeneratingContext,
  onCancelScan,
  onAddTemporaryScanPattern,
  onRemoveTemporaryScanPattern,
  onPreviewRun,
  onGeneratePrompt,
  onSaveProjectPrompt,
  onSelectProjectPrompt,
  onPromptChange,
  prompt,
  onSetFileIncluded,
  onStartScan,
  project,
  scanReport,
  temporaryScanPatterns,
  tab,
  setTab,
  onDeleteApiProfile,
  onFetchApiModels,
  onGetApiProfileSecret,
  onPutApiProfileSecret,
  onSaveApiProfile,
  onTestApiProfile,
  onUpdateProjectRunSettings,
  runHistory,
  runResultsError,
  runTasks,
  selectedRunId,
  selectedTaskId,
  taskDetails,
  isLoadingRunResults,
  onLoadTaskDetail,
  onLoadTaskRequestPreview,
  onOpenResult,
  onRetryTask,
  onRetryTasks,
  onSelectRun,
  retryBlocked,
  retryingTaskIds,
  isBatchRetrying,
  batchRetryTargetCount,
}: {
  apiProfileError: string | null;
  apiProfiles: readonly ApiProfileSummaryDto[];
  activeRun: ActiveRunSummary | null;
  fileRecords: readonly FileRecordSummaryDto[];
  fileTotal: number;
  contextVersion: ContextVersionDto | null;
  onAuthorizeSensitiveFile: (fileId: string) => Promise<void>;
  onRelocateProject: () => Promise<void>;
  isRelocatingProject: boolean;
  onGenerateContext: () => Promise<void> | void;
  isGeneratingContext: boolean;
  onCancelScan: () => void;
  onAddTemporaryScanPattern: (pattern: string) => void;
  onRemoveTemporaryScanPattern: (pattern: string) => void;
  onPreviewRun: () => void;
  onGeneratePrompt: (goal: string) => Promise<string>;
  onSaveProjectPrompt: (request: ProjectPromptSaveRequest) => Promise<void>;
  onSelectProjectPrompt: (request: ProjectPromptSelectRequest) => Promise<void>;
  onPromptChange: (value: string) => void;
  prompt: string;
  onSetFileIncluded: (fileId: string, included: boolean) => Promise<void>;
  onStartScan: () => void;
  project: ShellProject | null;
  scanReport: ScanReportDto | null;
  temporaryScanPatterns: readonly string[];
  tab: WorkspaceTab;
  setTab: (tab: WorkspaceTab) => void;
  onDeleteApiProfile: (id: string) => Promise<void>;
  onFetchApiModels: (id: string) => Promise<void>;
  onGetApiProfileSecret: (profileId: string) => Promise<string>;
  onPutApiProfileSecret: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onSaveApiProfile: (
    request: ApiProfileSaveRequest,
  ) => Promise<ApiProfileSummaryDto>;
  onTestApiProfile: (id: string) => Promise<void>;
  onUpdateProjectRunSettings: (
    request: ProjectRunSettingsUpdateRequest,
  ) => Promise<void>;
  runHistory: readonly RunSummaryDto[];
  runResultsError: string | null;
  runTasks: readonly TaskSummaryDto[];
  selectedRunId: string | null;
  selectedTaskId: string | null;
  taskDetails: Readonly<Record<string, TaskGetResponse>>;
  isLoadingRunResults: boolean;
  onLoadTaskDetail: (taskId: string) => Promise<void>;
  onLoadTaskRequestPreview: (
    taskId: string,
  ) => Promise<TaskRequestPreviewResponse>;
  onOpenResult: (taskId: string) => Promise<void>;
  onRetryTask: (taskId: string) => Promise<void>;
  onRetryTasks: (taskIds: readonly string[]) => Promise<void>;
  onSelectRun: (runId: string) => void;
  retryBlocked: boolean;
  retryingTaskIds: readonly string[];
  isBatchRetrying: boolean;
  batchRetryTargetCount: number;
}) {
  return (
    <main className="project-workspace">
      <ProjectHeader
        activeRun={activeRun}
        isRelocatingProject={isRelocatingProject}
        onRelocateProject={onRelocateProject}
        project={project}
      />
      <div className="workspace-tabs" role="tablist" aria-label="项目工作区">
        <TabButton
          active={tab === "prompt"}
          icon={<Sparkles size={15} />}
          label="提示词"
          onClick={() => setTab("prompt")}
        />
        <TabButton
          active={tab === "api"}
          icon={<Settings2 size={15} />}
          label="API 配置"
          onClick={() => setTab("api")}
        />
      </div>
      {tab === "prompt" ? (
        <PromptWorkspace
          fileRecords={fileRecords}
          fileTotal={fileTotal}
          contextVersion={contextVersion}
          onAuthorizeSensitiveFile={onAuthorizeSensitiveFile}
          onGenerateContext={onGenerateContext}
          isGeneratingContext={isGeneratingContext}
          onCancelScan={onCancelScan}
          onAddTemporaryScanPattern={onAddTemporaryScanPattern}
          onRemoveTemporaryScanPattern={onRemoveTemporaryScanPattern}
          onPreviewRun={onPreviewRun}
          onGeneratePrompt={onGeneratePrompt}
          onSaveProjectPrompt={onSaveProjectPrompt}
          onSelectProjectPrompt={onSelectProjectPrompt}
          onPromptChange={onPromptChange}
          prompt={prompt}
          onSetFileIncluded={onSetFileIncluded}
          onStartScan={onStartScan}
          project={project}
          scanReport={scanReport}
          temporaryScanPatterns={temporaryScanPatterns}
          runHistory={runHistory}
          runResultsError={runResultsError}
          runTasks={runTasks}
          selectedRunId={selectedRunId}
          selectedTaskId={selectedTaskId}
          taskDetails={taskDetails}
          isLoadingRunResults={isLoadingRunResults}
          onLoadTaskDetail={onLoadTaskDetail}
          onLoadTaskRequestPreview={onLoadTaskRequestPreview}
          onOpenResult={onOpenResult}
          onRetryTask={onRetryTask}
          onRetryTasks={onRetryTasks}
          onSelectRun={onSelectRun}
          retryBlocked={retryBlocked}
          retryingTaskIds={retryingTaskIds}
          isBatchRetrying={isBatchRetrying}
          batchRetryTargetCount={batchRetryTargetCount}
        />
      ) : (
        <ApiWorkspace
          error={apiProfileError}
          onDelete={onDeleteApiProfile}
          onFetchModels={onFetchApiModels}
          onGetSecret={onGetApiProfileSecret}
          onPutSecret={onPutApiProfileSecret}
          onSave={onSaveApiProfile}
          onTest={onTestApiProfile}
          profiles={apiProfiles}
          project={project}
          onUpdateProjectRunSettings={onUpdateProjectRunSettings}
        />
      )}
    </main>
  );
}

function ProjectHeader({
  activeRun,
  isRelocatingProject,
  onRelocateProject,
  project,
}: {
  activeRun: ActiveRunSummary | null;
  isRelocatingProject: boolean;
  onRelocateProject: () => Promise<void>;
  project: ShellProject | null;
}) {
  if (!project) {
    return (
      <section className="project-header project-header-empty">
        <div className="header-icon" aria-hidden="true">
          <LayoutGrid size={20} />
        </div>
        <div>
          <p className="eyebrow">CURRENT PROJECT</p>
          <h1>选择或添加一个项目</h1>
          <p>项目配置、文件和运行记录会在这里集中管理。</p>
        </div>
      </section>
    );
  }
  const isActive = activeRun?.projectId === project.id;
  return (
    <section className="project-header">
      <div className="header-icon" aria-hidden="true">
        <GitBranch size={20} />
      </div>
      <div className="project-header-copy">
        <div className="project-title-line">
          <p className="eyebrow">CURRENT PROJECT</p>
          <PathStatus status={project.pathStatus} />
          {isActive ? <span className="active-run-chip">活动 Run</span> : null}
        </div>
        <h1>{project.name}</h1>
        <p className="project-root">
          {project.rootDirectory ?? "仓库路径由 Rust 核心管理"}
        </p>
      </div>
      <button
        className="outline-button"
        disabled={isRelocatingProject}
        onClick={() => void onRelocateProject()}
        type="button"
      >
        <FolderPlus aria-hidden="true" size={15} />
        重新定位
      </button>
    </section>
  );
}

function TabButton({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-selected={active}
      className={`workspace-tab${active ? " is-active" : ""}`}
      onClick={onClick}
      role="tab"
      type="button"
    >
      {icon}
      {label}
    </button>
  );
}

function PromptWorkspace({
  contextVersion,
  fileRecords,
  fileTotal,
  onAuthorizeSensitiveFile,
  onGenerateContext,
  isGeneratingContext,
  onCancelScan,
  onAddTemporaryScanPattern,
  onRemoveTemporaryScanPattern,
  onPreviewRun,
  onGeneratePrompt,
  onSaveProjectPrompt,
  onSelectProjectPrompt,
  onPromptChange,
  onSetFileIncluded,
  onStartScan,
  project,
  prompt,
  scanReport,
  temporaryScanPatterns,
  runHistory,
  runResultsError,
  runTasks,
  selectedRunId,
  selectedTaskId,
  taskDetails,
  isLoadingRunResults,
  onLoadTaskDetail,
  onLoadTaskRequestPreview,
  onOpenResult,
  onRetryTask,
  onRetryTasks,
  onSelectRun,
  retryBlocked,
  retryingTaskIds,
  isBatchRetrying,
  batchRetryTargetCount,
}: {
  contextVersion: ContextVersionDto | null;
  onCancelScan: () => void;
  onAuthorizeSensitiveFile: (fileId: string) => Promise<void>;
  onGenerateContext: () => Promise<void> | void;
  isGeneratingContext: boolean;
  onAddTemporaryScanPattern: (pattern: string) => void;
  onRemoveTemporaryScanPattern: (pattern: string) => void;
  fileRecords: readonly FileRecordSummaryDto[];
  fileTotal: number;
  onPreviewRun: () => void;
  onGeneratePrompt: (goal: string) => Promise<string>;
  onSaveProjectPrompt: (request: ProjectPromptSaveRequest) => Promise<void>;
  onSelectProjectPrompt: (request: ProjectPromptSelectRequest) => Promise<void>;
  onPromptChange: (value: string) => void;
  onSetFileIncluded: (fileId: string, included: boolean) => Promise<void>;
  onStartScan: () => void;
  project: ShellProject | null;
  prompt: string;
  scanReport: ScanReportDto | null;
  temporaryScanPatterns: readonly string[];
  runHistory: readonly RunSummaryDto[];
  runResultsError: string | null;
  runTasks: readonly TaskSummaryDto[];
  selectedRunId: string | null;
  selectedTaskId: string | null;
  taskDetails: Readonly<Record<string, TaskGetResponse>>;
  isLoadingRunResults: boolean;
  onLoadTaskDetail: (taskId: string) => Promise<void>;
  onLoadTaskRequestPreview: (
    taskId: string,
  ) => Promise<TaskRequestPreviewResponse>;
  onOpenResult: (taskId: string) => Promise<void>;
  onRetryTask: (taskId: string) => Promise<void>;
  onRetryTasks: (taskIds: readonly string[]) => Promise<void>;
  onSelectRun: (runId: string) => void;
  retryBlocked: boolean;
  retryingTaskIds: readonly string[];
  isBatchRetrying: boolean;
  batchRetryTargetCount: number;
}) {
  const hasProject = project !== null;
  const [generatorOpen, setGeneratorOpen] = useState(false);
  const [generatorGoal, setGeneratorGoal] = useState("");
  const [generatorCandidate, setGeneratorCandidate] = useState("");
  const [generatorError, setGeneratorError] = useState<string | null>(null);
  const [isGeneratingPrompt, setIsGeneratingPrompt] = useState(false);
  const [promptNameDraft, setPromptNameDraft] = useState({
    projectId: null as string | null,
    value: "新的提示词",
  });
  const [promptSaveError, setPromptSaveError] = useState<string | null>(null);
  const promptPresets = project?.promptPresets ?? [];
  const activePromptId = project?.activePromptId ?? "";
  const activePreset = promptPresets.find(
    (preset) => preset.id === activePromptId,
  );
  const promptName =
    promptNameDraft.projectId === project?.id
      ? promptNameDraft.value
      : (activePreset?.name ?? "新的提示词");
  const isEditingActivePreset = activePreset?.name === promptName.trim();
  const setPromptName = (value: string) => {
    setPromptNameDraft({ projectId: project?.id ?? null, value });
  };
  const includedFileCount = fileRecords.length
    ? fileRecords.filter((file) => file.included).length
    : (scanReport?.includedFiles ?? 0);
  const selectableFiles = fileRecords.filter(
    (file) => file.included || canIncludeFile(file),
  );
  const allFilesIncluded =
    selectableFiles.length > 0 &&
    selectableFiles.every((file) => file.included);
  const [isUpdatingAllFiles, setIsUpdatingAllFiles] = useState(false);
  const [runTaskSort, setRunTaskSort] = useState<RunTaskSort>(null);
  const toggleRunTaskSort = (key: RunTaskSortKey) => {
    setRunTaskSort((current) => ({
      direction:
        current?.key === key && current.direction === "asc" ? "desc" : "asc",
      key,
    }));
  };
  const toggleAllFiles = async () => {
    if (isUpdatingAllFiles || selectableFiles.length === 0) return;
    const included = !allFilesIncluded;
    setIsUpdatingAllFiles(true);
    try {
      const filesToUpdate = selectableFiles.filter(
        (file) => file.included !== included,
      );
      for (
        let start = 0;
        start < filesToUpdate.length;
        start += BULK_FILE_UPDATE_BATCH_SIZE
      ) {
        const batch = filesToUpdate.slice(
          start,
          start + BULK_FILE_UPDATE_BATCH_SIZE,
        );
        await Promise.allSettled(
          batch.map((file) => onSetFileIncluded(file.id, included)),
        );
      }
    } catch {
      // The application layer owns the user-facing error.
    } finally {
      setIsUpdatingAllFiles(false);
    }
  };
  return (
    <div className="workspace-content">
      <div className="content-intro">
        <div>
          <p className="eyebrow">ANALYSIS BRIEF</p>
          <h2>先定义这次分析要回答的问题</h2>
        </div>
        <span className="content-counter">{prompt.length} 字符</span>
      </div>
      <section className="prompt-band">
        <div className="prompt-band-heading">
          <label htmlFor="project-prompt">项目默认提示词</label>
          <span>{promptPresets.length} 个常用提示词</span>
        </div>
        <div className="prompt-library-controls">
          <label htmlFor="prompt-name">常用提示词名称</label>
          <input
            disabled={!hasProject}
            id="prompt-name"
            onChange={(event) => setPromptName(event.target.value)}
            placeholder="例如：架构说明"
            value={promptName}
          />
          <label htmlFor="prompt-preset-select">选择常用提示词</label>
          <select
            disabled={!hasProject || promptPresets.length === 0}
            id="prompt-preset-select"
            onChange={(event) => {
              const nextId = event.target.value;
              const preset = promptPresets.find((item) => item.id === nextId);
              if (!preset || !project) return;
              setPromptSaveError(null);
              void onSelectProjectPrompt({
                projectId: project.id,
                promptId: preset.id,
              })
                .then(() => {
                  onPromptChange(preset.prompt);
                  setPromptName(preset.name);
                })
                .catch((error: unknown) => {
                  setPromptSaveError(
                    error instanceof Error ? error.message : "提示词选择失败。",
                  );
                });
            }}
            value={activePromptId}
          >
            <option value="">当前编辑（未保存）</option>
            {promptPresets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
        </div>
        <textarea
          disabled={!hasProject}
          id="project-prompt"
          onChange={(event) => onPromptChange(event.target.value)}
          value={prompt}
        />
        <div className="prompt-actions">
          <span>
            {promptSaveError ??
              (hasProject
                ? "保存后可在所有项目的下拉菜单中切换"
                : "添加项目后可以编辑提示词")}
          </span>
          <div>
            <button
              className="outline-button"
              disabled={!hasProject || !promptName.trim() || !prompt.trim()}
              onClick={() => {
                if (!project) return;
                setPromptSaveError(null);
                void onSaveProjectPrompt({
                  projectId: project.id,
                  name: promptName,
                  prompt,
                }).catch((error: unknown) => {
                  setPromptSaveError(
                    error instanceof Error ? error.message : "提示词保存失败。",
                  );
                });
              }}
              type="button"
            >
              {isEditingActivePreset
                ? "保存常用提示词修改"
                : "保存为项目默认并加入常用"}
            </button>
            <button
              className="primary-button"
              disabled={!hasProject}
              onClick={() => {
                setGeneratorError(null);
                setGeneratorOpen(true);
              }}
              type="button"
            >
              <Sparkles size={15} />
              生成提示词
            </button>
          </div>
        </div>
      </section>
      {generatorOpen ? (
        <PromptGeneratorPanel
          candidate={generatorCandidate}
          error={generatorError}
          goal={generatorGoal}
          isGenerating={isGeneratingPrompt}
          onChangeCandidate={setGeneratorCandidate}
          onChangeGoal={setGeneratorGoal}
          onClose={() => setGeneratorOpen(false)}
          onGenerate={async () => {
            setGeneratorError(null);
            setIsGeneratingPrompt(true);
            try {
              const generated = await onGeneratePrompt(generatorGoal);
              setGeneratorCandidate(generated);
            } catch (error) {
              setGeneratorError(
                error instanceof Error
                  ? error.message
                  : "提示词生成失败，请重试。",
              );
            } finally {
              setIsGeneratingPrompt(false);
            }
          }}
          onUse={() => {
            if (!generatorCandidate.trim()) return;
            onPromptChange(generatorCandidate.trim());
            setGeneratorOpen(false);
          }}
        />
      ) : null}
      <ContextPanel
        contextVersion={contextVersion}
        isGenerating={isGeneratingContext}
        onGenerate={() => void onGenerateContext()}
      />
      <ScanRuleEditor
        onAddPattern={onAddTemporaryScanPattern}
        onRemovePattern={onRemoveTemporaryScanPattern}
        report={scanReport?.rules ?? null}
        temporaryPatterns={temporaryScanPatterns}
      />
      <section className="task-area-band">
        <div className="task-area-heading">
          <div>
            <p className="eyebrow">TASK AREA</p>
            <h2>文件任务</h2>
          </div>
          <div className="task-area-actions">
            <button
              className="outline-button"
              disabled={!hasProject}
              onClick={
                scanReport?.status === "running" ? onCancelScan : onStartScan
              }
              type="button"
            >
              {scanReport?.status === "running" ? (
                <RefreshCw size={15} />
              ) : (
                <FolderPlus size={15} />
              )}
              {scanReport?.status === "running" ? "取消扫描" : "扫描仓库"}
            </button>
            <button
              aria-busy={isUpdatingAllFiles}
              className="outline-button"
              disabled={
                !hasProject ||
                selectableFiles.length === 0 ||
                isUpdatingAllFiles
              }
              onClick={() => void toggleAllFiles()}
              title={
                allFilesIncluded
                  ? "取消选择所有可纳入文件"
                  : "选择所有可纳入文件"
              }
              type="button"
            >
              {allFilesIncluded ? (
                <ListX aria-hidden="true" size={15} />
              ) : (
                <ListChecks aria-hidden="true" size={15} />
              )}
              {allFilesIncluded ? "取消全选文件" : "全选文件"}
            </button>
            <button
              className="primary-button"
              disabled={!hasProject}
              onClick={onPreviewRun}
              type="button"
            >
              <LayoutGrid size={15} />
              预览结果
            </button>
          </div>
        </div>
        <div className="scan-summary">
          <SummaryMetric
            label="已纳入文件"
            value={
              scanReport?.status === "completed" || fileTotal > 0
                ? `${includedFileCount}`
                : "—"
            }
          />
          <SummaryMetric label="待处理" value="—" />
          <SummaryMetric label="最近 Run" value="—" />
          <div className="scan-summary-note">
            <CircleAlert size={15} />
            {scanSummaryMessage(scanReport)}
          </div>
        </div>
        <FileTreeTable
          emptyLabel={
            scanReport?.status === "completed"
              ? "没有符合当前扫描规则的文件"
              : hasProject
                ? "扫描项目后，文件任务会显示在这里"
                : "添加项目后，文件任务会显示在这里"
          }
          files={fileRecords}
          onAuthorizeSensitive={onAuthorizeSensitiveFile}
          onSetIncluded={async (file, included) =>
            onSetFileIncluded(file.id, included)
          }
        />
      </section>
      <RunResultsPanel
        error={runResultsError}
        isLoading={isLoadingRunResults}
        key={selectedRunId ?? "no-run"}
        onLoadTaskDetail={onLoadTaskDetail}
        onLoadTaskRequestPreview={onLoadTaskRequestPreview}
        onOpenResult={onOpenResult}
        onRetryTask={onRetryTask}
        onRetryTasks={onRetryTasks}
        onSelectRun={onSelectRun}
        onToggleSort={toggleRunTaskSort}
        retryBlocked={retryBlocked}
        retryingTaskIds={retryingTaskIds}
        isBatchRetrying={isBatchRetrying}
        batchRetryTargetCount={batchRetryTargetCount}
        runHistory={runHistory}
        sort={runTaskSort}
        runTasks={runTasks}
        selectedRunId={selectedRunId}
        selectedTaskId={selectedTaskId}
        taskDetails={taskDetails}
      />
    </div>
  );
}

function RunResultsPanel({
  error,
  isLoading,
  onLoadTaskDetail,
  onLoadTaskRequestPreview,
  onOpenResult,
  onRetryTask,
  onRetryTasks,
  onSelectRun,
  onToggleSort,
  retryBlocked,
  retryingTaskIds,
  isBatchRetrying,
  batchRetryTargetCount,
  runHistory,
  sort,
  runTasks,
  selectedRunId,
  selectedTaskId,
  taskDetails,
}: {
  error: string | null;
  isLoading: boolean;
  onLoadTaskDetail: (taskId: string) => Promise<void>;
  onLoadTaskRequestPreview: (
    taskId: string,
  ) => Promise<TaskRequestPreviewResponse>;
  onOpenResult: (taskId: string) => Promise<void>;
  onRetryTask: (taskId: string) => Promise<void>;
  onRetryTasks: (taskIds: readonly string[]) => Promise<void>;
  onSelectRun: (runId: string) => void;
  onToggleSort: (key: RunTaskSortKey) => void;
  retryBlocked: boolean;
  retryingTaskIds: readonly string[];
  isBatchRetrying: boolean;
  batchRetryTargetCount: number;
  runHistory: readonly RunSummaryDto[];
  sort: RunTaskSort;
  runTasks: readonly TaskSummaryDto[];
  selectedRunId: string | null;
  selectedTaskId: string | null;
  taskDetails: Readonly<Record<string, TaskGetResponse>>;
}) {
  const [requestPreview, setRequestPreview] =
    useState<TaskRequestPreviewResponse | null>(null);
  const [loadingPromptTaskId, setLoadingPromptTaskId] = useState<string | null>(
    null,
  );
  const sortedTasks = useMemo(
    () => sortRunTasks(runTasks, sort),
    [runTasks, sort],
  );
  const selectedTask = runTasks.find((task) => task.id === selectedTaskId);
  const selectedDetail = selectedTaskId ? taskDetails[selectedTaskId] : null;
  const failedTaskIds = runTasks
    .filter((task) => task.status === "failed")
    .map((task) => task.id);
  const retryingTaskIdSet = new Set(retryingTaskIds);
  const activeRetryTaskId = retryingTaskIds[0] ?? null;
  const displayedBatchCount = isBatchRetrying
    ? batchRetryTargetCount || failedTaskIds.length
    : failedTaskIds.length;
  const selectRun = (runId: string) => {
    setRequestPreview(null);
    setLoadingPromptTaskId(null);
    onSelectRun(runId);
  };
  const openPrompt = async (taskId: string) => {
    setLoadingPromptTaskId(taskId);
    try {
      const preview = await onLoadTaskRequestPreview(taskId);
      setRequestPreview(preview);
    } catch {
      setRequestPreview(null);
    } finally {
      setLoadingPromptTaskId((current) =>
        current === taskId ? null : current,
      );
    }
  };
  return (
    <section className="run-results-panel" aria-label="运行结果">
      <div className="run-results-heading">
        <div>
          <p className="eyebrow">RUN RESULTS</p>
          <h2>运行结果</h2>
        </div>
        <div className="run-results-heading-actions">
          {displayedBatchCount > 0 ? (
            <button
              aria-busy={isBatchRetrying}
              className="outline-button retry-failed-button"
              disabled={
                retryBlocked || isBatchRetrying || retryingTaskIds.length > 0
              }
              onClick={() => void onRetryTasks(failedTaskIds)}
              type="button"
            >
              <RefreshCw
                aria-hidden="true"
                className={isBatchRetrying ? "is-spinning" : undefined}
                size={14}
              />
              {isBatchRetrying
                ? `批量重试中（${displayedBatchCount}）`
                : `重试全部失败（${displayedBatchCount}）`}
            </button>
          ) : null}
          {isLoading ? (
            <span className="run-results-loading">正在刷新</span>
          ) : null}
        </div>
      </div>
      {error ? (
        <div className="project-error" role="alert">
          {error}
        </div>
      ) : null}
      {runHistory.length ? (
        <label className="run-selector">
          选择 Run
          <select
            aria-label="选择 Run"
            onChange={(event) => selectRun(event.target.value)}
            value={selectedRunId ?? ""}
          >
            {runHistory.map((run) => (
              <option key={run.id} value={run.id}>
                {formatRunLabel(run)}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <p className="run-results-empty">当前项目还没有正式 Run。</p>
      )}
      {runHistory.length ? (
        <>
          <div className="run-results-stats">
            {runHistory.find((run) => run.id === selectedRunId)?.stats &&
              renderRunStats(
                runHistory.find((run) => run.id === selectedRunId)!.stats,
              )}
          </div>
          <VirtualTaskTable
            ariaLabel="运行结果任务"
            className="run-results-task-table"
            emptyLabel="该 Run 没有 Task"
            getRowKey={(task) => task.id}
            header={
              <>
                <span role="columnheader">文件</span>
                <SortableTaskHeader
                  activeSort={sort}
                  label="状态"
                  onSort={() => onToggleSort("status")}
                  sortKey="status"
                />
                <SortableTaskHeader
                  activeSort={sort}
                  label="请求时间"
                  onSort={() => onToggleSort("startedAt")}
                  sortKey="startedAt"
                />
                <span role="columnheader">模型</span>
                <span role="columnheader">提示词</span>
                <span role="columnheader">结果 / Attempt</span>
              </>
            }
            items={sortedTasks}
            renderRow={(task) => {
              const detail = taskDetails[task.id];
              return (
                <>
                  <span
                    className="run-task-path"
                    role="cell"
                    title={task.relativePath}
                  >
                    {task.relativePath}
                  </span>
                  <span role="cell">{taskStatusLabel(task.status)}</span>
                  <span className="run-task-request-time" role="cell">
                    {formatTaskStartedAt(task.startedAt)}
                  </span>
                  <span role="cell" title={task.modelSnapshot}>
                    {task.modelSnapshot}
                  </span>
                  <span className="run-task-prompt" role="cell">
                    <button
                      aria-busy={loadingPromptTaskId === task.id}
                      aria-label={`查看提示词 ${task.relativePath}`}
                      className="text-button"
                      disabled={loadingPromptTaskId === task.id}
                      onClick={() => void openPrompt(task.id)}
                      title="查看发送给 AI 的提示词"
                      type="button"
                    >
                      查看提示词
                    </button>
                  </span>
                  <span className="run-task-actions" role="cell">
                    {task.hasResult || task.status === "failed" ? (
                      <button
                        aria-label={`查看结果 ${task.relativePath}`}
                        className="text-button"
                        onClick={() => void onOpenResult(task.id)}
                        type="button"
                      >
                        查看结果
                      </button>
                    ) : (
                      <span className="file-tree-muted">无结果</span>
                    )}
                    <button
                      className="text-button"
                      onClick={() => void onLoadTaskDetail(task.id)}
                      type="button"
                    >
                      {detail ? `${detail.attempts.length} 次尝试` : "查看尝试"}
                    </button>
                    {task.status === "failed" ? (
                      <button
                        aria-label={`重试 ${task.relativePath}`}
                        className="text-button retry-task-button"
                        disabled={
                          retryBlocked ||
                          isBatchRetrying ||
                          retryingTaskIdSet.has(task.id)
                        }
                        onClick={() => void onRetryTask(task.id)}
                        type="button"
                      >
                        <RefreshCw
                          aria-hidden="true"
                          className={
                            activeRetryTaskId === task.id
                              ? "is-spinning"
                              : undefined
                          }
                          size={14}
                        />
                        {activeRetryTaskId === task.id
                          ? "重试中"
                          : retryingTaskIdSet.has(task.id)
                            ? "已排队"
                            : "重试"}
                      </button>
                    ) : null}
                  </span>
                </>
              );
            }}
          />
          {selectedTask && selectedDetail ? (
            <AttemptDetailPanel
              attempts={selectedDetail.attempts}
              path={selectedTask.relativePath}
            />
          ) : null}
          {requestPreview ? (
            <PromptDetailPanel
              input={requestPreview.input}
              instructions={requestPreview.instructions}
              onClose={() => setRequestPreview(null)}
              path={requestPreview.task.relativePath}
            />
          ) : null}
        </>
      ) : null}
    </section>
  );
}

type RunTaskSortKey = "status" | "startedAt";
type RunTaskSort = {
  key: RunTaskSortKey;
  direction: "asc" | "desc";
} | null;

const TASK_STATUS_SORT_ORDER: Record<TaskSummaryDto["status"], number> = {
  pending: 0,
  queued: 1,
  running: 2,
  succeeded: 3,
  failed: 4,
  cancelled: 5,
  interrupted: 6,
  source_changed: 7,
};

function SortableTaskHeader({
  activeSort,
  label,
  onSort,
  sortKey,
}: {
  activeSort: RunTaskSort;
  label: string;
  onSort: () => void;
  sortKey: RunTaskSortKey;
}) {
  const direction = activeSort?.key === sortKey ? activeSort.direction : null;
  const nextDirection = direction === "asc" ? "desc" : "asc";
  const SortIcon =
    direction === "asc"
      ? ArrowUp
      : direction === "desc"
        ? ArrowDown
        : ArrowUpDown;
  const nextDirectionLabel = nextDirection === "asc" ? "升序" : "降序";

  return (
    <span
      aria-sort={
        direction === "asc"
          ? "ascending"
          : direction === "desc"
            ? "descending"
            : undefined
      }
      className="task-table-column-header"
      role="columnheader"
    >
      <button
        aria-label={`按${label}${nextDirectionLabel}排序`}
        className={`task-table-sort-button${direction ? " is-active" : ""}`}
        onClick={onSort}
        title={`按${label}${nextDirectionLabel}排序`}
        type="button"
      >
        <span>{label}</span>
        <SortIcon aria-hidden="true" size={13} />
      </button>
    </span>
  );
}

function sortRunTasks(
  tasks: readonly TaskSummaryDto[],
  sort: RunTaskSort,
): readonly TaskSummaryDto[] {
  if (!sort) return tasks;

  return tasks
    .map((task, originalIndex) => ({ originalIndex, task }))
    .sort((left, right) => {
      const comparison =
        sort.key === "status"
          ? (TASK_STATUS_SORT_ORDER[left.task.status] -
              TASK_STATUS_SORT_ORDER[right.task.status]) *
            (sort.direction === "asc" ? 1 : -1)
          : compareStartedAt(
              left.task.startedAt,
              right.task.startedAt,
              sort.direction,
            );
      if (comparison === 0) return left.originalIndex - right.originalIndex;
      return comparison;
    })
    .map(({ task }) => task);
}

function compareStartedAt(
  left: string | null,
  right: string | null,
  direction: "asc" | "desc",
): number {
  const leftTime = parseTimestamp(left);
  const rightTime = parseTimestamp(right);
  if (leftTime === null && rightTime === null) return 0;
  if (leftTime === null) return 1;
  if (rightTime === null) return -1;
  return (leftTime - rightTime) * (direction === "asc" ? 1 : -1);
}

function parseTimestamp(value: string | null): number | null {
  if (value === null) return null;
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? null : timestamp;
}

function AttemptDetailPanel({
  attempts,
  path,
}: {
  attempts: readonly AttemptDto[];
  path: string;
}) {
  return (
    <div className="attempt-detail-panel">
      <div className="attempt-detail-heading">
        <strong>{path}</strong>
        <span>{attempts.length} 次请求尝试</span>
      </div>
      {attempts.length ? (
        <div className="attempt-list">
          {attempts.map((attempt) => (
            <div className="attempt-row" key={attempt.id}>
              <span>#{attempt.sequence}</span>
              <span>{attemptStatusLabel(attempt.status)}</span>
              <span>{attempt.apiProfileName}</span>
              <span>{attempt.actualModel}</span>
              <span>{attempt.totalTokens ?? "—"} tokens</span>
              <span>{attempt.error?.message ?? "—"}</span>
            </div>
          ))}
        </div>
      ) : (
        <span className="file-tree-muted">尚未产生请求尝试</span>
      )}
    </div>
  );
}

function FailureResultPreview({
  detail,
  onClose,
}: {
  detail: TaskGetResponse;
  onClose: () => void;
}) {
  const latestFailure = findLatestFailedAttempt(detail.attempts);
  const latestReason = latestFailure
    ? attemptFailureReason(latestFailure)
    : "任务失败，但没有记录可用的请求尝试。";
  return (
    <div className="preview-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        aria-label={`失败结果：${detail.task.relativePath}`}
        aria-modal="true"
        className="preview-dialog"
        role="dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="preview-dialog-header">
          <div>
            <p className="eyebrow">FAILED RESULT</p>
            <h2>分析失败</h2>
            <span
              className="prompt-detail-path"
              title={detail.task.relativePath}
            >
              {detail.task.relativePath}
            </span>
          </div>
          <button
            aria-label="关闭失败结果"
            className="icon-button"
            onClick={onClose}
            title="关闭失败结果"
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </div>
        <div className="failure-result-content">
          <div className="failure-result-summary" role="alert">
            <CircleAlert aria-hidden="true" size={20} />
            <div>
              <strong>失败原因</strong>
              <p>{latestReason}</p>
              {latestFailure?.error ? (
                <code>{latestFailure.error.code}</code>
              ) : null}
            </div>
          </div>
          <div className="failure-attempts-heading">
            <h3>请求尝试</h3>
            <span>{detail.attempts.length} 次</span>
          </div>
          {detail.attempts.length ? (
            <div className="failure-attempt-list">
              {detail.attempts.map((attempt) => (
                <div className="failure-attempt-row" key={attempt.id}>
                  <div className="failure-attempt-title">
                    <strong>第 {attempt.sequence} 次</strong>
                    <span>{attemptStatusLabel(attempt.status)}</span>
                  </div>
                  <dl className="failure-attempt-meta">
                    <div>
                      <dt>API 档案</dt>
                      <dd>{attempt.apiProfileName}</dd>
                    </div>
                    <div>
                      <dt>模型</dt>
                      <dd>{attempt.actualModel}</dd>
                    </div>
                    <div>
                      <dt>HTTP 状态</dt>
                      <dd>{attempt.httpStatus ?? "无"}</dd>
                    </div>
                    <div>
                      <dt>耗时</dt>
                      <dd>
                        {attempt.durationMs === null
                          ? "无"
                          : `${attempt.durationMs} ms`}
                      </dd>
                    </div>
                  </dl>
                  <p className="failure-attempt-reason">
                    {attemptFailureReason(attempt)}
                  </p>
                  {attempt.error ? (
                    <code className="failure-error-code">
                      {attempt.error.code}
                    </code>
                  ) : null}
                </div>
              ))}
            </div>
          ) : (
            <p className="file-tree-muted">没有可显示的 Attempt 记录。</p>
          )}
        </div>
      </section>
    </div>
  );
}

function findLatestFailedAttempt(
  attempts: readonly AttemptDto[],
): AttemptDto | null {
  for (let index = attempts.length - 1; index >= 0; index -= 1) {
    const attempt = attempts[index];
    if (attempt?.error) return attempt;
  }
  return attempts[attempts.length - 1] ?? null;
}

function attemptFailureReason(attempt: AttemptDto): string {
  const error = attempt.error;
  if (!error) return "本次尝试没有记录失败详情。";
  const knownReasons: Readonly<Record<string, string>> = {
    output_write_failed: "模型已返回内容，但结果文件写入失败。",
    project_path_unavailable: "项目路径不可用，无法读取目标文件。",
    provider_authentication_failed: "API Key 无效或模型服务认证失败。",
    provider_cancelled: "模型请求已取消。",
    provider_connection_failed: "无法连接模型服务。",
    provider_content_rejected: "模型服务拒绝了请求内容。",
    provider_interrupted_unknown: "模型请求被中断，结果状态未知。",
    provider_invalid_request: "模型服务拒绝了请求参数。",
    provider_invalid_response: "模型服务返回了无法识别的响应。",
    provider_model_unavailable: "请求的模型不可用。",
    provider_permission_denied: "API Profile 没有访问该模型的权限。",
    provider_rate_limited: "模型服务触发限流。",
    provider_server_error: "模型服务暂时不可用。",
    provider_timeout: "模型请求超时。",
    scan_file_unreadable: "目标文件无法读取。",
    security_secret_store_unavailable: "API Key 安全存储不可用。",
  };
  const knownReason = knownReasons[error.code];
  if (knownReason) return knownReason;
  if (error.sanitized && error.message.trim()) return error.message;
  return "请求失败，但没有可安全显示的详细原因。";
}

function PromptDetailPanel({
  input,
  instructions,
  onClose,
  path,
}: {
  input: string;
  instructions: string;
  onClose: () => void;
  path: string;
}) {
  return (
    <div className="preview-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        aria-label={`发送给 AI 的提示词：${path}`}
        aria-modal="true"
        className="preview-dialog"
        role="dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="preview-dialog-header">
          <div>
            <p className="eyebrow">REQUEST PROMPT</p>
            <h2>发送给 AI 的提示词</h2>
            <span className="prompt-detail-path" title={path}>
              {path}
            </span>
          </div>
          <button
            aria-label="关闭提示词预览"
            className="icon-button"
            onClick={onClose}
            title="关闭提示词预览"
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </div>
        <pre className="prompt-detail-content">
          {`[INSTRUCTIONS]\n${instructions}${instructions ? "\n\n" : "\n"}[INPUT]\n${input}`}
        </pre>
      </section>
    </div>
  );
}

function formatRunLabel(run: RunSummaryDto): string {
  return `${formatRunCreatedAt(run.createdAt) ?? run.id} · ${historyRunStatusLabel(run.status)}`;
}

function formatTaskStartedAt(startedAt: string | null): string {
  if (startedAt === null) return "—";
  const date = new Date(startedAt);
  if (Number.isNaN(date.getTime())) return "—";

  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function formatRunCreatedAt(createdAt: string): string | null {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) return null;

  const parts = new Intl.DateTimeFormat("zh-CN", {
    day: "2-digit",
    hour: "2-digit",
    hour12: false,
    minute: "2-digit",
    month: "2-digit",
    second: "2-digit",
    timeZone: "Asia/Shanghai",
    year: "numeric",
  }).formatToParts(date);
  const part = (type: string) =>
    parts.find((item) => item.type === type)?.value;
  const year = part("year");
  const month = part("month");
  const day = part("day");
  const hour = part("hour");
  const minute = part("minute");
  const second = part("second");
  if (!year || !month || !day || !hour || !minute || !second) return null;

  return `${year}-${month}-${day} ${hour}:${minute}:${second}`;
}

function renderRunStats(stats: RunSummaryDto["stats"]) {
  return (
    <>
      <SummaryMetric label="总任务" value={String(stats.total)} />
      <SummaryMetric label="成功" value={String(stats.succeeded)} />
      <SummaryMetric label="失败" value={String(stats.failed)} />
      <SummaryMetric label="处理中" value={String(stats.running)} />
    </>
  );
}

function taskStatusLabel(status: TaskSummaryDto["status"]): string {
  return {
    pending: "待处理",
    queued: "排队中",
    running: "处理中",
    succeeded: "成功",
    failed: "失败",
    cancelled: "已取消",
    interrupted: "已中断",
    source_changed: "源文件已变化",
  }[status];
}

function attemptStatusLabel(status: AttemptDto["status"]): string {
  return {
    created: "已创建",
    dispatched: "已发送",
    succeeded: "成功",
    failed_retryable: "可重试失败",
    failed_terminal: "失败",
    cancelled: "已取消",
    interrupted_unknown: "已中断",
  }[status];
}

function historyRunStatusLabel(status: RunSummaryDto["status"]): string {
  return {
    draft: "草稿",
    running: "运行中",
    pausing: "暂停中",
    paused: "已暂停",
    cancelling: "取消中",
    cancelled: "已取消",
    completed: "已完成",
    completed_with_errors: "完成但有错误",
    interrupted: "已中断",
  }[status];
}

function RunPreviewPanel({
  error,
  isCreating,
  onClose,
  onCreate,
  preview,
}: {
  error: string | null;
  isCreating: boolean;
  onClose: () => void;
  onCreate: () => void;
  preview: RunPreviewResponse | null;
}) {
  if (!preview) return null;
  const canCreate = preview.blockers.length === 0 && !isCreating;
  return (
    <div className="run-preview-backdrop">
      <section
        aria-labelledby="run-preview-title"
        aria-modal="true"
        className="run-preview-panel"
        role="dialog"
      >
        <div className="run-preview-heading">
          <div>
            <p className="eyebrow">RUN PREVIEW</p>
            <h2 id="run-preview-title">确认本次分析</h2>
          </div>
          <button
            aria-label="关闭 Run 预览"
            className="icon-button"
            onClick={onClose}
            title="关闭"
            type="button"
          >
            <X aria-hidden="true" size={16} />
          </button>
        </div>
        <div className="run-preview-summary">
          <SummaryMetric
            label="目标文件"
            value={String(preview.tasks.length)}
          />
          <SummaryMetric label="模型" value={preview.model ?? "未配置"} />
          <SummaryMetric label="并发数" value={String(preview.concurrency)} />
          <SummaryMetric
            label="提示词"
            value={
              preview.promptSource === "override" ? "当前编辑" : "项目默认"
            }
          />
        </div>
        {preview.blockers.length ? (
          <div className="run-preview-blockers" role="alert">
            <strong>暂不能创建 Run</strong>
            {preview.blockers.map((blocker, index) => (
              <div key={blocker.code + (blocker.relativePath ?? "") + index}>
                <span>{blocker.message}</span>
                {blocker.relativePath ? (
                  <code>{blocker.relativePath}</code>
                ) : null}
              </div>
            ))}
          </div>
        ) : (
          <div className="run-preview-ready" role="status">
            确认后将为每个目标文件创建一个 queued Task，并立即开始发送模型请求。
          </div>
        )}
        {error ? (
          <div className="project-error api-error" role="alert">
            {error}
          </div>
        ) : null}
        <div className="run-preview-files">
          {preview.tasks.slice(0, 12).map((task) => (
            <div className="run-preview-file" key={task.fileId}>
              <span>{task.relativePath}</span>
              <small>{formatBytes(task.sizeBytes)}</small>
            </div>
          ))}
          {preview.tasks.length > 12 ? (
            <small>还有 {preview.tasks.length - 12} 个文件</small>
          ) : null}
        </div>
        <div className="run-preview-actions">
          <button className="outline-button" onClick={onClose} type="button">
            返回修改
          </button>
          <button
            className="primary-button"
            disabled={!canCreate}
            onClick={onCreate}
            type="button"
          >
            {isCreating ? "正在启动分析" : "创建并开始分析"}
          </button>
        </div>
      </section>
    </div>
  );
}

function formatBytes(value: number | bigint): string {
  const bytes = Number(value);
  if (bytes < 1024) return String(bytes) + " B";
  if (bytes < 1024 * 1024) {
    return String(Math.round(bytes / 1024)) + " KB";
  }
  return (bytes / (1024 * 1024)).toFixed(1) + " MB";
}

function SummaryMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="summary-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function PromptGeneratorPanel({
  candidate,
  error,
  goal,
  isGenerating,
  onChangeCandidate,
  onChangeGoal,
  onClose,
  onGenerate,
  onUse,
}: {
  candidate: string;
  error: string | null;
  goal: string;
  isGenerating: boolean;
  onChangeCandidate: (value: string) => void;
  onChangeGoal: (value: string) => void;
  onClose: () => void;
  onGenerate: () => Promise<void>;
  onUse: () => void;
}) {
  return (
    <div className="prompt-generator-backdrop">
      <section
        aria-labelledby="prompt-generator-title"
        aria-modal="true"
        className="prompt-generator-panel"
        role="dialog"
      >
        <div className="prompt-generator-heading">
          <div>
            <p className="eyebrow">PROMPT BUILDER</p>
            <h2 id="prompt-generator-title">生成提示词</h2>
          </div>
          <button
            aria-label="关闭提示词生成"
            className="icon-button"
            onClick={onClose}
            title="关闭提示词生成"
            type="button"
          >
            <X aria-hidden="true" size={17} />
          </button>
        </div>
        <label htmlFor="prompt-generator-goal">这次分析希望回答什么问题</label>
        <textarea
          id="prompt-generator-goal"
          onChange={(event) => onChangeGoal(event.target.value)}
          placeholder="例如：梳理核心模块的职责、数据流和修改风险"
          value={goal}
        />
        {error ? (
          <div className="prompt-generator-error" role="alert">
            {error}
          </div>
        ) : null}
        <div className="prompt-generator-actions">
          <button className="outline-button" onClick={onClose} type="button">
            返回编辑
          </button>
          <button
            className="primary-button"
            disabled={isGenerating || !goal.trim()}
            onClick={() => void onGenerate()}
            type="button"
          >
            <Sparkles aria-hidden="true" size={15} />
            {isGenerating ? "正在生成" : "生成候选"}
          </button>
        </div>
        {candidate ? (
          <>
            <label htmlFor="prompt-generator-candidate">候选提示词</label>
            <textarea
              id="prompt-generator-candidate"
              onChange={(event) => onChangeCandidate(event.target.value)}
              value={candidate}
            />
            <div className="prompt-generator-actions prompt-generator-confirm">
              <span>候选内容仍可继续编辑</span>
              <button
                className="primary-button"
                disabled={!candidate.trim()}
                onClick={onUse}
                type="button"
              >
                使用此提示词
              </button>
            </div>
          </>
        ) : null}
      </section>
    </div>
  );
}

function ContextPanel({
  contextVersion,
  isGenerating,
  onGenerate,
}: {
  contextVersion: ContextVersionDto | null;
  isGenerating: boolean;
  onGenerate: () => void;
}) {
  return (
    <section className="context-panel">
      <div className="context-panel-heading">
        <div>
          <p className="eyebrow">PROJECT CONTEXT</p>
          <h2>项目上下文</h2>
        </div>
        <button
          className="outline-button"
          disabled={isGenerating}
          onClick={onGenerate}
          type="button"
        >
          <RefreshCw size={14} />
          {isGenerating ? "正在发现" : contextVersion ? "重新发现" : "发现资料"}
        </button>
      </div>
      {contextVersion ? (
        <>
          <div className="context-panel-meta">
            <span>{contextVersion.sourceFiles.length} 个来源文件</span>
            <span>
              {contextVersion.status === "ready" ? "已就绪" : "需要处理"}
            </span>
            <code>{contextVersion.id}</code>
          </div>
          <div className="context-source-list">
            {contextVersion.sourceFiles.length ? (
              contextVersion.sourceFiles.map((source) => (
                <div className="context-source-item" key={source.relativePath}>
                  <span>{source.relativePath}</span>
                  <code>{source.contentHash.slice(0, 15)}...</code>
                </div>
              ))
            ) : (
              <span className="context-empty">未发现 README 或 AGENTS.md</span>
            )}
          </div>
          <p className="context-summary">{contextVersion.summary}</p>
        </>
      ) : (
        <p className="context-empty">
          尚未建立上下文版本。发现根目录 README 和 AGENTS.md 后，Run
          可以固定该版本。
        </p>
      )}
    </section>
  );
}

function ScanRuleEditor({
  onAddPattern,
  onRemovePattern,
  report,
  temporaryPatterns,
}: {
  onAddPattern: (pattern: string) => void;
  onRemovePattern: (pattern: string) => void;
  report: ScanRuleSummaryDto | null;
  temporaryPatterns: readonly string[];
}) {
  const [pattern, setPattern] = useState("");
  const addPattern = () => {
    const normalized = pattern.trim();
    if (!normalized) return;
    onAddPattern(normalized);
    setPattern("");
  };
  const rules = report ?? {
    builtinDirectories: [],
    builtinExtensions: [],
    gitignoreRules: [],
    temporaryExcludedPatterns: [],
    sensitiveDetectionEnabled: true,
  };

  return (
    <section className="scan-rules-band">
      <div className="scan-rules-heading">
        <div>
          <p className="eyebrow">SCAN RULES</p>
          <h2>排除规则</h2>
        </div>
        <span className="scan-rules-status">
          敏感检测：{rules.sensitiveDetectionEnabled ? "开启" : "关闭"}
        </span>
      </div>
      <div className="scan-rule-grid">
        <details open>
          <summary>内置目录 ({rules.builtinDirectories.length})</summary>
          <div className="scan-rule-values">
            {rules.builtinDirectories.length ? (
              rules.builtinDirectories.map((value) => (
                <code key={value}>{value}</code>
              ))
            ) : (
              <span className="scan-rule-empty">扫描后显示</span>
            )}
          </div>
        </details>
        <details>
          <summary>内置扩展名 ({rules.builtinExtensions.length})</summary>
          <div className="scan-rule-values">
            {rules.builtinExtensions.length ? (
              rules.builtinExtensions.map((value) => (
                <code key={value}>.{value}</code>
              ))
            ) : (
              <span className="scan-rule-empty">扫描后显示</span>
            )}
          </div>
        </details>
        <details>
          <summary>项目 .gitignore ({rules.gitignoreRules.length})</summary>
          <div className="scan-rule-values">
            {rules.gitignoreRules.length ? (
              rules.gitignoreRules.map((value, index) => (
                <code key={`${value}-${index}`}>{value}</code>
              ))
            ) : (
              <span className="scan-rule-empty">没有有效规则</span>
            )}
          </div>
        </details>
        <div className="scan-rule-temporary">
          <label htmlFor="temporary-scan-pattern">临时排除模式</label>
          <div className="scan-rule-input">
            <input
              id="temporary-scan-pattern"
              onChange={(event) => setPattern(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  addPattern();
                }
              }}
              placeholder="例如 docs/** 或 *.log"
              value={pattern}
            />
            <button
              aria-label="添加临时排除模式"
              className="icon-button"
              disabled={!pattern.trim()}
              onClick={addPattern}
              title="添加临时排除模式"
              type="button"
            >
              <Plus aria-hidden="true" size={16} />
            </button>
          </div>
          <div className="scan-rule-values scan-rule-temporary-values">
            {temporaryPatterns.length ? (
              temporaryPatterns.map((value) => (
                <span className="scan-rule-chip" key={value}>
                  <code>{value}</code>
                  <button
                    aria-label={`移除临时排除模式 ${value}`}
                    className="icon-button"
                    onClick={() => onRemovePattern(value)}
                    title={`移除临时排除模式 ${value}`}
                    type="button"
                  >
                    <X aria-hidden="true" size={12} />
                  </button>
                </span>
              ))
            ) : (
              <span className="scan-rule-empty">本次会话未添加</span>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}

function scanSummaryMessage(report: ScanReportDto | null): string {
  if (!report) return "尚未扫描项目；扫描结果将在这里显示。";
  switch (report.status) {
    case "running":
      return `正在扫描：已检查 ${report.scannedFiles} 个文件`;
    case "completed":
      return `扫描完成：纳入 ${report.includedFiles} 个文件`;
    case "cancelled":
      return "扫描已取消，本次结果未提交。";
    case "failed":
      return "扫描失败，请检查项目路径和权限。";
  }
}

function ApiWorkspace({
  error,
  onDelete,
  onFetchModels,
  onGetSecret,
  onPutSecret,
  onSave,
  onTest,
  profiles,
  project,
  onUpdateProjectRunSettings,
}: {
  error: string | null;
  onDelete: (id: string) => Promise<void>;
  onFetchModels: (id: string) => Promise<void>;
  onGetSecret: (profileId: string) => Promise<string>;
  onPutSecret: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onSave: (request: ApiProfileSaveRequest) => Promise<ApiProfileSummaryDto>;
  onTest: (id: string) => Promise<void>;
  profiles: readonly ApiProfileSummaryDto[];
  project: ShellProject | null;
  onUpdateProjectRunSettings: (
    request: ProjectRunSettingsUpdateRequest,
  ) => Promise<void>;
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creatingNew, setCreatingNew] = useState(false);
  const selected = creatingNew
    ? null
    : (profiles.find((profile) => profile.id === selectedId) ??
      profiles[0] ??
      null);

  const startNew = () => {
    setSelectedId(null);
    setCreatingNew(true);
  };

  return (
    <div className="workspace-content api-workspace">
      <div className="content-intro">
        <div>
          <p className="eyebrow">CONNECTIONS</p>
          <h2>API 配置档案</h2>
        </div>
        <button className="primary-button" onClick={startNew} type="button">
          <Plus size={15} />
          添加 API 档案
        </button>
      </div>
      {error ? (
        <div className="project-error api-error" role="alert">
          {error}
        </div>
      ) : null}
      <ProjectRunSettings
        key={`${project?.id ?? "none"}:${project?.primaryProfileId ?? "none"}:${project?.defaultModel ?? "none"}:${project?.concurrency ?? 3}:${profiles.map((profile) => `${profile.id}:${profile.defaultModel ?? "none"}:${profile.modelCache.length}`).join(",")}`}
        onUpdate={onUpdateProjectRunSettings}
        profiles={profiles}
        project={project}
      />
      <section className="api-config-band">
        <aside className="api-profile-list" aria-label="API Profile 列表">
          {profiles.length === 0 ? (
            <div className="api-profile-empty">
              <Settings2 aria-hidden="true" size={19} />
              <span>还没有 API Profile</span>
            </div>
          ) : (
            profiles.map((profile) => (
              <button
                className={`api-profile-item${profile.id === selected?.id ? " is-selected" : ""}`}
                key={profile.id}
                onClick={() => {
                  setCreatingNew(false);
                  setSelectedId(profile.id);
                }}
                type="button"
              >
                <strong>{profile.name}</strong>
                <span>{profile.baseUrl}</span>
                <small>{profile.hasSecret ? "已配置密钥" : "未配置密钥"}</small>
              </button>
            ))
          )}
        </aside>
        <ApiProfileEditor
          key={creatingNew ? "new" : (selected?.id ?? "new")}
          onDelete={onDelete}
          onFetchModels={onFetchModels}
          onGetSecret={onGetSecret}
          onPutSecret={onPutSecret}
          onSave={async (request) => {
            const profile = await onSave(request);
            setCreatingNew(false);
            setSelectedId(profile.id);
            return profile;
          }}
          onTest={onTest}
          profile={selected}
        />
      </section>
    </div>
  );
}

function ProjectRunSettings({
  onUpdate,
  profiles,
  project,
}: {
  onUpdate: (request: ProjectRunSettingsUpdateRequest) => Promise<void>;
  profiles: readonly ApiProfileSummaryDto[];
  project: ShellProject | null;
}) {
  const initialProfileId = project?.primaryProfileId ?? profiles[0]?.id ?? "";
  const initialProfile = profiles.find(
    (profile) => profile.id === initialProfileId,
  );
  const [primaryProfileId, setPrimaryProfileId] = useState(initialProfileId);
  const [defaultModel, setDefaultModel] = useState(
    project?.defaultModel ?? initialProfile?.defaultModel ?? "",
  );
  const [concurrencyInput, setConcurrencyInput] = useState(
    String(project?.concurrency ?? 3),
  );
  const [busy, setBusy] = useState(false);

  const selectedProfile = profiles.find(
    (profile) => profile.id === primaryProfileId,
  );
  const modelOptions = Array.from(
    new Set([
      ...(selectedProfile?.defaultModel ? [selectedProfile.defaultModel] : []),
      ...(selectedProfile?.modelCache.map((model) => model.id) ?? []),
    ]),
  );
  const concurrency = Number(concurrencyInput);
  const concurrencyIsValid =
    Number.isInteger(concurrency) && concurrency >= 1 && concurrency <= 30;
  const save = async () => {
    if (
      !project ||
      !primaryProfileId ||
      !defaultModel.trim() ||
      !concurrencyIsValid
    )
      return;
    setBusy(true);
    try {
      await onUpdate({
        projectId: project.id,
        primaryProfileId,
        defaultModel: defaultModel.trim(),
        concurrency,
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="project-run-settings">
      <div>
        <p className="eyebrow">PROJECT ROUTING</p>
        <h3>当前项目运行设置</h3>
      </div>
      <label>
        主 API Profile
        <select
          disabled={!project || profiles.length === 0 || busy}
          onChange={(event) => {
            const profileId = event.target.value;
            const profile = profiles.find((item) => item.id === profileId);
            setPrimaryProfileId(profileId);
            if (profile?.defaultModel) setDefaultModel(profile.defaultModel);
          }}
          value={primaryProfileId}
        >
          {profiles.length === 0 ? (
            <option value="">尚无 API Profile</option>
          ) : null}
          {profiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profile.name}
            </option>
          ))}
        </select>
      </label>
      <label>
        项目默认模型
        <input
          disabled={!project || !primaryProfileId || busy}
          list={project ? `project-models-${project.id}` : undefined}
          onChange={(event) => setDefaultModel(event.target.value)}
          placeholder="例如 gpt-5"
          value={defaultModel}
        />
        {project ? (
          <datalist id={`project-models-${project.id}`}>
            {modelOptions.map((model) => (
              <option key={model} value={model} />
            ))}
          </datalist>
        ) : null}
      </label>
      <label>
        并发请求数
        <input
          disabled={!project || busy}
          max={30}
          min={1}
          onChange={(event) => setConcurrencyInput(event.target.value)}
          step={1}
          type="number"
          value={concurrencyInput}
        />
      </label>
      <button
        className="primary-button"
        disabled={
          !project ||
          !primaryProfileId ||
          !defaultModel.trim() ||
          !concurrencyIsValid ||
          busy
        }
        onClick={() => void save()}
        type="button"
      >
        {busy ? "正在保存" : "保存项目运行设置"}
      </button>
    </section>
  );
}

function ApiProfileEditor({
  onDelete,
  onFetchModels,
  onGetSecret,
  onPutSecret,
  onSave,
  onTest,
  profile,
}: {
  onDelete: (id: string) => Promise<void>;
  onFetchModels: (id: string) => Promise<void>;
  onGetSecret: (profileId: string) => Promise<string>;
  onPutSecret: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onSave: (request: ApiProfileSaveRequest) => Promise<ApiProfileSummaryDto>;
  onTest: (id: string) => Promise<void>;
  profile: ApiProfileSummaryDto | null;
}) {
  const [name, setName] = useState(profile?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(
    profile?.baseUrl ?? "https://api.openai.com/v1",
  );
  const [defaultModel, setDefaultModel] = useState(profile?.defaultModel ?? "");
  const [secret, setSecret] = useState("");
  const [secretDirty, setSecretDirty] = useState(false);
  const [secretVisible, setSecretVisible] = useState(false);
  const [secretError, setSecretError] = useState<string | null>(null);
  const [revealingSecret, setRevealingSecret] = useState(false);
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      const savedProfile = await onSave({
        ...(profile ? { id: profile.id } : {}),
        baseUrl,
        defaultModel: defaultModel.trim() || null,
        name,
      });
      if (secretDirty && secret.trim()) {
        await onPutSecret({ profileId: savedProfile.id, secret });
      }
      setSecret("");
      setSecretDirty(false);
      setSecretVisible(false);
      setSecretError(null);
    } finally {
      setBusy(false);
    }
  };

  const test = async () => {
    if (!profile) return;
    setBusy(true);
    try {
      await onTest(profile.id);
    } finally {
      setBusy(false);
    }
  };

  const fetchModels = async () => {
    if (!profile) return;
    setBusy(true);
    try {
      await onFetchModels(profile.id);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!profile) return;
    setBusy(true);
    try {
      await onDelete(profile.id);
    } finally {
      setBusy(false);
    }
  };

  const toggleSecretVisibility = async () => {
    if (secretVisible) {
      setSecretVisible(false);
      if (!secretDirty) setSecret("");
      return;
    }
    if (secret) {
      setSecretVisible(true);
      return;
    }
    if (!profile?.hasSecret) return;
    setSecretError(null);
    setRevealingSecret(true);
    try {
      const value = await onGetSecret(profile.id);
      setSecret(value);
      setSecretDirty(false);
      setSecretVisible(true);
    } catch (error) {
      setSecretError(
        error instanceof Error ? error.message : "API Key 读取失败。",
      );
    } finally {
      setRevealingSecret(false);
    }
  };

  return (
    <div className="api-profile-form">
      <div className="api-form-heading">
        <div>
          <p className="eyebrow">PROFILE SETTINGS</p>
          <h3>{profile ? "编辑 API Profile" : "新建 API Profile"}</h3>
        </div>
        {profile ? (
          <span
            className={
              "api-profile-status api-profile-status-" +
              profile.lastConnectionStatus
            }
          >
            {apiConnectionStatusLabel(profile.lastConnectionStatus)}
          </span>
        ) : null}
      </div>
      <label>
        名称
        <input onChange={(event) => setName(event.target.value)} value={name} />
      </label>
      <label>
        Base URL
        <input
          onChange={(event) => setBaseUrl(event.target.value)}
          value={baseUrl}
        />
      </label>
      <label>
        默认模型
        <input
          onChange={(event) => setDefaultModel(event.target.value)}
          placeholder="例如 gpt-5"
          value={defaultModel}
        />
      </label>
      <label>
        API Key
        <div className="api-key-input">
          <input
            autoComplete="new-password"
            onChange={(event) => {
              setSecret(event.target.value);
              setSecretDirty(true);
              setSecretError(null);
            }}
            placeholder={
              profile?.hasSecret
                ? "已配置，可点击右侧按钮显示"
                : "写入操作系统安全存储"
            }
            type={secretVisible ? "text" : "password"}
            value={secret}
          />
          <button
            aria-label={secretVisible ? "隐藏 API Key" : "显示 API Key"}
            className="icon-button api-key-toggle"
            disabled={
              busy || revealingSecret || (!secret && !profile?.hasSecret)
            }
            onClick={() => void toggleSecretVisibility()}
            title={secretVisible ? "隐藏 API Key" : "显示 API Key"}
            type="button"
          >
            {secretVisible ? (
              <EyeOff aria-hidden="true" size={16} />
            ) : (
              <Eye aria-hidden="true" size={16} />
            )}
          </button>
        </div>
      </label>
      <div className="api-form-note">
        <span>
          {secretError ??
            (profile?.hasSecret
              ? "密钥由操作系统安全存储托管"
              : "尚未配置密钥")}
        </span>
        <span>协议：openai-responses</span>
      </div>
      <div className="api-form-actions">
        <button
          className="primary-button"
          disabled={busy}
          onClick={() => void save()}
          type="button"
        >
          保存配置
        </button>
        {profile ? (
          <>
            <button
              className="outline-button"
              disabled={busy}
              onClick={() => void test()}
              type="button"
            >
              测试连接
            </button>
            <button
              className="outline-button"
              disabled={busy}
              onClick={() => void fetchModels()}
              type="button"
            >
              刷新模型
            </button>
            <button
              className="danger-button"
              disabled={busy}
              onClick={() => void remove()}
              type="button"
            >
              删除
            </button>
          </>
        ) : null}
      </div>
      {profile?.modelCache.length ? (
        <div className="api-model-cache">
          <span>模型缓存</span>
          <div>
            {profile.modelCache.slice(0, 8).map((model) => (
              <span key={model.id}>{model.id}</span>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function apiConnectionStatusLabel(
  status: ApiProfileSummaryDto["lastConnectionStatus"],
): string {
  switch (status) {
    case "healthy":
      return "连接正常";
    case "failed":
      return "连接失败";
    default:
      return "未测试";
  }
}

function runStatusLabel(status: ActiveRunSummary["status"]) {
  return {
    running: "运行中",
    paused: "已暂停",
    cancelling: "取消中",
    interrupted: "已中断",
  }[status];
}
