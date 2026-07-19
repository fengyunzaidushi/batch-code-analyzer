import {
  Activity,
  ChevronRight,
  CircleAlert,
  FolderPlus,
  GitBranch,
  LayoutGrid,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  Sparkles,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import type {
  ApiProfileSaveRequest,
  ApiProfileSummaryDto,
  FileRecordSummaryDto,
  ProjectPathStatus,
  ProjectSummaryDto,
  ScanReportDto,
} from "@batch-code-analyzer/ipc-types";

import { MarkdownPreview } from "../features/markdown/MarkdownPreview";
import { FileTreeTable } from "../features/tasks/FileTreeTable";

export type ShellProject = ProjectSummaryDto & {
  rootDirectory?: string;
  runningTaskCount?: number;
  failedTaskCount?: number;
};

export type ShellHealthState =
  "checking" | "ready" | "degraded" | "unavailable";

export interface ActiveRunSummary {
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
  onAddProject?: () => void;
  onCancelScan?: () => void;
  onRetryHealth?: () => void;
  onSetFileIncluded?: (fileId: string, included: boolean) => Promise<void>;
  onDeleteApiProfile?: (id: string) => Promise<void>;
  onFetchApiModels?: (id: string) => Promise<void>;
  onPutApiProfileSecret?: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onSaveApiProfile?: (
    request: ApiProfileSaveRequest,
  ) => Promise<ApiProfileSummaryDto>;
  onTestApiProfile?: (id: string) => Promise<void>;
  onSelectProject?: (id: string) => void;
  onStartScan?: () => void;
  projectError?: string | null;
  scanReport?: ScanReportDto | null;
  selectedProjectId?: string | null;
  isAddingProject?: boolean;
}

type WorkspaceTab = "prompt" | "api";

export function AppShell({
  apiProfileError = null,
  apiProfiles = [],
  fileRecords = [],
  fileTotal = 0,
  projects = [],
  healthState = "checking",
  activeRun = null,
  onAddProject = () => undefined,
  onCancelScan = () => undefined,
  onRetryHealth = () => undefined,
  onSetFileIncluded = async () => undefined,
  onDeleteApiProfile = async () => undefined,
  onFetchApiModels = async () => undefined,
  onPutApiProfileSecret = async () => {
    throw new Error("API Profile secret handler is unavailable");
  },
  onSaveApiProfile = async () => {
    throw new Error("API Profile save handler is unavailable");
  },
  onTestApiProfile = async () => undefined,
  onSelectProject,
  onStartScan = () => undefined,
  projectError = null,
  scanReport = null,
  selectedProjectId: controlledSelectedProjectId,
  isAddingProject = false,
}: AppShellProps) {
  const [internalSelectedProjectId, setInternalSelectedProjectId] = useState<
    string | null
  >(projects[0]?.id ?? null);
  const [search, setSearch] = useState("");
  const [tab, setTab] = useState<WorkspaceTab>("prompt");
  const [previewOpen, setPreviewOpen] = useState(false);
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

  return (
    <div className="desktop-shell">
      <GlobalRunBar
        activeRun={activeRun}
        healthState={healthState}
        onRetryHealth={onRetryHealth}
      />
      <div className="shell-body">
        <ProjectSidebar
          activeRun={activeRun}
          onAddProject={onAddProject}
          onSelect={selectProject}
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
          onCancelScan={onCancelScan}
          onPreview={() => setPreviewOpen(true)}
          onSetFileIncluded={onSetFileIncluded}
          onStartScan={onStartScan}
          project={selectedProject}
          scanReport={scanReport}
          tab={tab}
          setTab={setTab}
          apiProfileError={apiProfileError}
          apiProfiles={apiProfiles}
          onDeleteApiProfile={onDeleteApiProfile}
          onFetchApiModels={onFetchApiModels}
          onPutApiProfileSecret={onPutApiProfileSecret}
          onSaveApiProfile={onSaveApiProfile}
          onTestApiProfile={onTestApiProfile}
        />
      </div>
      <MarkdownPreview
        content={"# 结果预览\n\n完成后，安全的 Markdown 结果会在这里打开。"}
        onClose={() => setPreviewOpen(false)}
        open={previewOpen}
      />
    </div>
  );
}

function GlobalRunBar({
  activeRun,
  healthState,
  onRetryHealth,
}: {
  activeRun: ActiveRunSummary | null;
  healthState: ShellHealthState;
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
        <button className="sidebar-footer-action" type="button">
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
  onCancelScan,
  onPreview,
  onSetFileIncluded,
  onStartScan,
  project,
  scanReport,
  tab,
  setTab,
  onDeleteApiProfile,
  onFetchApiModels,
  onPutApiProfileSecret,
  onSaveApiProfile,
  onTestApiProfile,
}: {
  apiProfileError: string | null;
  apiProfiles: readonly ApiProfileSummaryDto[];
  activeRun: ActiveRunSummary | null;
  fileRecords: readonly FileRecordSummaryDto[];
  fileTotal: number;
  onCancelScan: () => void;
  onPreview: () => void;
  onSetFileIncluded: (fileId: string, included: boolean) => Promise<void>;
  onStartScan: () => void;
  project: ShellProject | null;
  scanReport: ScanReportDto | null;
  tab: WorkspaceTab;
  setTab: (tab: WorkspaceTab) => void;
  onDeleteApiProfile: (id: string) => Promise<void>;
  onFetchApiModels: (id: string) => Promise<void>;
  onPutApiProfileSecret: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onSaveApiProfile: (
    request: ApiProfileSaveRequest,
  ) => Promise<ApiProfileSummaryDto>;
  onTestApiProfile: (id: string) => Promise<void>;
}) {
  return (
    <main className="project-workspace">
      <ProjectHeader activeRun={activeRun} project={project} />
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
          onCancelScan={onCancelScan}
          onPreview={onPreview}
          onSetFileIncluded={onSetFileIncluded}
          onStartScan={onStartScan}
          project={project}
          scanReport={scanReport}
        />
      ) : (
        <ApiWorkspace
          error={apiProfileError}
          onDelete={onDeleteApiProfile}
          onFetchModels={onFetchApiModels}
          onPutSecret={onPutApiProfileSecret}
          onSave={onSaveApiProfile}
          onTest={onTestApiProfile}
          profiles={apiProfiles}
        />
      )}
    </main>
  );
}

function ProjectHeader({
  activeRun,
  project,
}: {
  activeRun: ActiveRunSummary | null;
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
        disabled={project.pathStatus === "unavailable"}
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
  fileRecords,
  fileTotal,
  onCancelScan,
  onPreview,
  onSetFileIncluded,
  onStartScan,
  project,
  scanReport,
}: {
  onCancelScan: () => void;
  fileRecords: readonly FileRecordSummaryDto[];
  fileTotal: number;
  onPreview: () => void;
  onSetFileIncluded: (fileId: string, included: boolean) => Promise<void>;
  onStartScan: () => void;
  project: ShellProject | null;
  scanReport: ScanReportDto | null;
}) {
  const [prompt, setPrompt] = useState(
    "请结合提供的项目上下文，用通俗但准确的语言解释当前代码文件。\n\n请说明核心职责、关键输入输出、协作模块和修改影响。",
  );
  const hasProject = project !== null;
  const includedFileCount =
    scanReport?.status === "completed"
      ? scanReport.includedFiles
      : fileRecords.filter((file) => file.included).length;
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
        <label htmlFor="project-prompt">项目默认提示词</label>
        <textarea
          disabled={!hasProject}
          id="project-prompt"
          onChange={(event) => setPrompt(event.target.value)}
          value={prompt}
        />
        <div className="prompt-actions">
          <span>
            {hasProject
              ? "未保存的编辑只在当前会话保留"
              : "添加项目后可以编辑提示词"}
          </span>
          <div>
            <button className="outline-button" disabled type="button">
              保存为项目默认
            </button>
            <button
              className="primary-button"
              disabled={!hasProject}
              type="button"
            >
              <Sparkles size={15} />
              生成提示词
            </button>
          </div>
        </div>
      </section>
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
              className="primary-button"
              disabled={!hasProject}
              onClick={onPreview}
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
          onSetIncluded={async (file, included) =>
            onSetFileIncluded(file.id, included)
          }
        />
      </section>
    </div>
  );
}

function SummaryMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="summary-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
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
  onPutSecret,
  onSave,
  onTest,
  profiles,
}: {
  error: string | null;
  onDelete: (id: string) => Promise<void>;
  onFetchModels: (id: string) => Promise<void>;
  onPutSecret: (request: {
    profileId: string;
    secret: string;
  }) => Promise<ApiProfileSummaryDto>;
  onSave: (request: ApiProfileSaveRequest) => Promise<ApiProfileSummaryDto>;
  onTest: (id: string) => Promise<void>;
  profiles: readonly ApiProfileSummaryDto[];
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

function ApiProfileEditor({
  onDelete,
  onFetchModels,
  onPutSecret,
  onSave,
  onTest,
  profile,
}: {
  onDelete: (id: string) => Promise<void>;
  onFetchModels: (id: string) => Promise<void>;
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
      if (secret.trim()) {
        await onPutSecret({ profileId: savedProfile.id, secret });
        setSecret("");
      }
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
        <input
          autoComplete="new-password"
          onChange={(event) => setSecret(event.target.value)}
          placeholder={
            profile?.hasSecret
              ? "已配置，输入新值可替换"
              : "只写入安全存储，不会回显"
          }
          type="password"
          value={secret}
        />
      </label>
      <div className="api-form-note">
        <span>
          {profile?.hasSecret ? "密钥已由会话安全存储托管" : "尚未配置密钥"}
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
