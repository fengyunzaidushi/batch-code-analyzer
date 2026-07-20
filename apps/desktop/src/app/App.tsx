import { useCallback, useEffect, useState } from "react";
import type {
  ApiProfileSaveRequest,
  ApiProfileSummaryDto,
  ResultReadResponse,
  ContextVersionDto,
  FileRecordSummaryDto,
  IpcError,
  ProjectRunSettingsUpdateRequest,
  ProjectSummaryDto,
  RunPreviewRequest,
  RunPreviewResponse,
  RunSummaryDto,
  ScanReportDto,
  TaskGetResponse,
  TaskSummaryDto,
} from "@batch-code-analyzer/ipc-types";

import {
  addProject,
  chooseProjectDirectory,
  getProject,
  listProjects,
  updateProjectRunSettings,
} from "../ipc/projects";
import { checkBackendHealth } from "../ipc/health";
import { generateContext, getContext } from "../ipc/context";
import {
  authorizeSensitiveFile,
  listFiles,
  setFileIncluded,
} from "../ipc/files";
import {
  cancelScan,
  getScanReport,
  startScan,
  subscribeScanProgress,
} from "../ipc/scan";
import {
  deleteApiProfile,
  fetchApiModels,
  listApiProfiles,
  putApiProfileSecret,
  saveApiProfile,
  testApiProfile,
  type ApiProfileSecretPutRequest,
} from "../ipc/apiProfiles";
import {
  createRun,
  executeRun,
  getTask,
  listRuns,
  listTasks,
  previewRun,
  readResult,
} from "../ipc/runs";
import { AppShell, type ShellHealthState, type ShellProject } from "./AppShell";

export function App() {
  const [healthState, setHealthState] = useState<ShellHealthState>("checking");
  const [requestId, setRequestId] = useState(0);
  const [projects, setProjects] = useState<ShellProject[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    null,
  );
  const [projectError, setProjectError] = useState<string | null>(null);
  const [isAddingProject, setIsAddingProject] = useState(false);
  const [scanReports, setScanReports] = useState<Record<string, ScanReportDto>>(
    {},
  );
  const [temporaryScanPatterns, setTemporaryScanPatterns] = useState<
    Record<string, string[]>
  >({});
  const [fileRecords, setFileRecords] = useState<
    Record<string, FileRecordSummaryDto[]>
  >({});
  const [fileTotals, setFileTotals] = useState<Record<string, number>>({});
  const [contexts, setContexts] = useState<
    Record<string, ContextVersionDto | null>
  >({});
  const [isGeneratingContext, setIsGeneratingContext] = useState(false);
  const [apiProfiles, setApiProfiles] = useState<ApiProfileSummaryDto[]>([]);
  const [apiProfileError, setApiProfileError] = useState<string | null>(null);
  const [activeRun, setActiveRun] = useState<{
    projectId: string;
    projectName: string;
    status: "running";
  } | null>(null);
  const [runPreview, setRunPreview] = useState<RunPreviewResponse | null>(null);
  const [runPreparation, setRunPreparation] = useState<
    Pick<RunPreviewRequest, "prompt" | "model">
  >({});
  const [runError, setRunError] = useState<string | null>(null);
  const [isCreatingRun, setIsCreatingRun] = useState(false);
  const [runHistory, setRunHistory] = useState<RunSummaryDto[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [runTasks, setRunTasks] = useState<TaskSummaryDto[]>([]);
  const [taskDetails, setTaskDetails] = useState<
    Record<string, TaskGetResponse>
  >({});
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [runResultsError, setRunResultsError] = useState<string | null>(null);
  const [isLoadingRunResults, setIsLoadingRunResults] = useState(false);
  const [resultPreview, setResultPreview] = useState<ResultReadResponse | null>(
    null,
  );

  const refreshProjects = useCallback(async () => {
    const items = await listProjects();
    setProjects(items.map(toShellProject));
    setSelectedProjectId((current) =>
      items.some((item) => item.id === current)
        ? current
        : (items[0]?.id ?? null),
    );
  }, []);

  const refreshFiles = useCallback(async (projectId: string) => {
    const response = await listFiles(projectId);
    setFileRecords((current) => ({
      ...current,
      [projectId]: response.items,
    }));
    setFileTotals((current) => ({
      ...current,
      [projectId]: response.total,
    }));
  }, []);

  const refreshRunHistory = useCallback(
    async (projectId: string, preferredRunId?: string) => {
      setIsLoadingRunResults(true);
      try {
        const response = await listRuns({ projectId, limit: 500 });
        setRunHistory(response.items);
        setSelectedRunId((current) => {
          if (
            preferredRunId &&
            response.items.some((run) => run.id === preferredRunId)
          ) {
            return preferredRunId;
          }
          if (current && response.items.some((run) => run.id === current)) {
            return current;
          }
          return response.items[0]?.id ?? null;
        });
        setRunResultsError(null);
      } catch (error) {
        setRunResultsError(safeRunResultsError(error));
        setRunHistory([]);
        setSelectedRunId(null);
        setRunTasks([]);
      } finally {
        setIsLoadingRunResults(false);
      }
    },
    [],
  );

  useEffect(() => {
    let active = true;
    void checkBackendHealth().then(
      (response) => {
        if (active) setHealthState(response.status);
      },
      () => {
        if (active) setHealthState("unavailable");
      },
    );
    return () => {
      active = false;
    };
  }, [requestId]);

  useEffect(() => {
    let active = true;
    void listProjects().then(
      (items) => {
        if (!active) return;
        setProjects(items.map(toShellProject));
        setSelectedProjectId((current) =>
          items.some((item) => item.id === current)
            ? current
            : (items[0]?.id ?? null),
        );
      },
      (error) => {
        if (!active) return;
        setProjects([]);
        setProjectError(safeProjectError(error));
      },
    );
    return () => {
      active = false;
    };
  }, [refreshProjects]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void subscribeScanProgress((report) => {
      if (!active) return;
      setScanReports((current) => ({
        ...current,
        [report.projectId]: report,
      }));
      if (report.status === "completed") {
        void refreshFiles(report.projectId).catch((error) => {
          if (active) setProjectError(safeProjectError(error));
        });
      }
    }).then((cleanup) => {
      if (active) {
        unlisten = cleanup;
      } else {
        cleanup();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [refreshFiles]);

  useEffect(() => {
    if (!selectedProjectId) return;
    let active = true;
    void getProject(selectedProjectId).then(
      (detail) => {
        if (!active) return;
        setProjects((current) =>
          current.map((project) =>
            project.id === detail.id
              ? {
                  ...project,
                  rootDirectory: detail.sourceDirectory,
                  primaryProfileId: detail.apiRouting.primaryProfileId,
                  defaultModel: detail.defaultModel,
                }
              : project,
          ),
        );
      },
      () => undefined,
    );
    return () => {
      active = false;
    };
  }, [selectedProjectId]);

  useEffect(() => {
    if (!selectedProjectId) {
      return;
    }
    let active = true;
    void listRuns({ projectId: selectedProjectId, limit: 500 }).then(
      (response) => {
        if (!active) return;
        setRunHistory(response.items);
        setSelectedRunId((current) =>
          current && response.items.some((run) => run.id === current)
            ? current
            : (response.items[0]?.id ?? null),
        );
        setRunResultsError(null);
      },
      (error) => {
        if (!active) return;
        setRunResultsError(safeRunResultsError(error));
        setRunHistory([]);
        setSelectedRunId(null);
        setRunTasks([]);
      },
    );
    return () => {
      active = false;
    };
  }, [selectedProjectId]);

  useEffect(() => {
    if (!selectedProjectId || !selectedRunId) {
      return;
    }
    let active = true;
    void listTasks({
      projectId: selectedProjectId,
      runId: selectedRunId,
      limit: 500,
    }).then(
      (response) => {
        if (!active) return;
        setRunTasks(response.items);
        setSelectedTaskId(null);
        setRunResultsError(null);
      },
      (error) => {
        if (!active) return;
        setRunTasks([]);
        setRunResultsError(safeRunResultsError(error));
      },
    );
    return () => {
      active = false;
    };
  }, [selectedProjectId, selectedRunId]);

  useEffect(() => {
    if (!selectedProjectId) return;
    let active = true;
    void listFiles(selectedProjectId).then(
      (response) => {
        if (!active) return;
        setFileRecords((current) => ({
          ...current,
          [selectedProjectId]: response.items,
        }));
        setFileTotals((current) => ({
          ...current,
          [selectedProjectId]: response.total,
        }));
      },
      (error) => {
        if (active) setProjectError(safeProjectError(error));
      },
    );
    return () => {
      active = false;
    };
  }, [refreshFiles, selectedProjectId]);

  useEffect(() => {
    if (!selectedProjectId) return;
    let active = true;
    void getContext(selectedProjectId).then(
      (response) => {
        if (active) {
          setContexts((current) => ({
            ...current,
            [selectedProjectId]: response.context,
          }));
        }
      },
      (error) => {
        if (active) setProjectError(safeProjectError(error));
      },
    );
    return () => {
      active = false;
    };
  }, [selectedProjectId]);

  useEffect(() => {
    let active = true;
    void listApiProfiles().then(
      (response) => {
        if (active) setApiProfiles(response.items);
      },
      (error) => {
        if (active) setApiProfileError(safeApiProfileError(error));
      },
    );
    return () => {
      active = false;
    };
  }, []);

  const handleAddProject = async () => {
    setProjectError(null);
    setIsAddingProject(true);
    try {
      const sourceDirectory = await chooseProjectDirectory();
      if (!sourceDirectory) return;
      const response = await addProject({ sourceDirectory });
      await refreshProjects();
      setSelectedProjectId(response.project.id);
      if (response.configMirrorWarning) {
        setProjectError("项目已登记，但仓库配置镜像未能写入。");
      }
    } catch (error) {
      setProjectError(safeProjectError(error));
    } finally {
      setIsAddingProject(false);
    }
  };

  const handleAddTemporaryScanPattern = (pattern: string) => {
    if (!selectedProjectId) return;
    const normalized = pattern.trim();
    if (!normalized) return;
    setTemporaryScanPatterns((current) => {
      const patterns = current[selectedProjectId] ?? [];
      if (patterns.includes(normalized) || patterns.length >= 100)
        return current;
      return {
        ...current,
        [selectedProjectId]: [...patterns, normalized],
      };
    });
  };

  const handleRemoveTemporaryScanPattern = (pattern: string) => {
    if (!selectedProjectId) return;
    setTemporaryScanPatterns((current) => ({
      ...current,
      [selectedProjectId]: (current[selectedProjectId] ?? []).filter(
        (value) => value !== pattern,
      ),
    }));
  };

  const handleStartScan = async () => {
    if (!selectedProjectId) return;
    setProjectError(null);
    try {
      const response = await startScan(
        selectedProjectId,
        temporaryScanPatterns[selectedProjectId] ?? [],
      );
      const report = await getScanReport(response.operationId);
      setScanReports((current) => ({
        ...current,
        [selectedProjectId]: report,
      }));
    } catch (error) {
      setProjectError(safeProjectError(error));
    }
  };

  const handleSetFileIncluded = useCallback(
    async (fileId: string, included: boolean) => {
      if (!selectedProjectId) return;
      setProjectError(null);
      try {
        const response = await setFileIncluded(
          selectedProjectId,
          fileId,
          included,
        );
        setFileRecords((current) => ({
          ...current,
          [selectedProjectId]: (current[selectedProjectId] ?? []).map((file) =>
            file.id === response.file.id ? response.file : file,
          ),
        }));
      } catch (error) {
        setProjectError(safeProjectError(error));
        throw error;
      }
    },
    [selectedProjectId],
  );

  const handleAuthorizeSensitiveFile = useCallback(
    async (fileId: string) => {
      if (!selectedProjectId) return;
      setProjectError(null);
      try {
        const response = await authorizeSensitiveFile(
          selectedProjectId,
          fileId,
        );
        setFileRecords((current) => ({
          ...current,
          [selectedProjectId]: (current[selectedProjectId] ?? []).map((file) =>
            file.id === response.file.id ? response.file : file,
          ),
        }));
      } catch (error) {
        setProjectError(safeProjectError(error));
        throw error;
      }
    },
    [selectedProjectId],
  );

  const handleCancelScan = async () => {
    if (!selectedProjectId) return;
    const report = scanReports[selectedProjectId];
    if (!report || report.status !== "running") return;
    try {
      await cancelScan(report.operationId);
    } catch (error) {
      setProjectError(safeProjectError(error));
    }
  };

  const handleGenerateContext = async () => {
    if (!selectedProjectId) return;
    setProjectError(null);
    setIsGeneratingContext(true);
    try {
      const response = await generateContext(selectedProjectId);
      setContexts((current) => ({
        ...current,
        [selectedProjectId]: response.context,
      }));
    } catch (error) {
      setProjectError(safeProjectError(error));
    } finally {
      setIsGeneratingContext(false);
    }
  };

  const handleSaveApiProfile = async (
    request: ApiProfileSaveRequest,
  ): Promise<ApiProfileSummaryDto> => {
    setApiProfileError(null);
    try {
      const response = await saveApiProfile(request);
      setApiProfiles((current) => {
        const existing = current.some(
          (profile) => profile.id === response.profile.id,
        );
        return existing
          ? current.map((profile) =>
              profile.id === response.profile.id ? response.profile : profile,
            )
          : [...current, response.profile];
      });
      return response.profile;
    } catch (error) {
      setApiProfileError(safeApiProfileError(error));
      throw error;
    }
  };

  const handlePutApiProfileSecret = async (
    request: ApiProfileSecretPutRequest,
  ): Promise<ApiProfileSummaryDto> => {
    setApiProfileError(null);
    try {
      const response = await putApiProfileSecret(request);
      setApiProfiles((current) =>
        current.map((profile) =>
          profile.id === response.profile.id ? response.profile : profile,
        ),
      );
      return response.profile;
    } catch (error) {
      setApiProfileError(safeApiProfileError(error));
      throw error;
    }
  };

  const handleTestApiProfile = async (id: string) => {
    setApiProfileError(null);
    try {
      const response = await testApiProfile({ id });
      setApiProfiles((current) =>
        current.map((profile) =>
          profile.id === response.profile.id ? response.profile : profile,
        ),
      );
    } catch (error) {
      setApiProfileError(safeApiProfileError(error));
    }
  };

  const handleFetchApiModels = async (id: string) => {
    setApiProfileError(null);
    try {
      const response = await fetchApiModels({ id });
      setApiProfiles((current) =>
        current.map((profile) =>
          profile.id === response.profile.id ? response.profile : profile,
        ),
      );
    } catch (error) {
      setApiProfileError(safeApiProfileError(error));
    }
  };

  const handleDeleteApiProfile = async (id: string) => {
    setApiProfileError(null);
    try {
      await deleteApiProfile({ id });
      setApiProfiles((current) =>
        current.filter((profile) => profile.id !== id),
      );
    } catch (error) {
      setApiProfileError(safeApiProfileError(error));
      throw error;
    }
  };

  const handleUpdateProjectRunSettings = async (
    request: ProjectRunSettingsUpdateRequest,
  ) => {
    setApiProfileError(null);
    try {
      const response = await updateProjectRunSettings(request);
      setProjects((current) =>
        current.map((project) =>
          project.id === response.project.id
            ? {
                ...project,
                rootDirectory: response.project.sourceDirectory,
                primaryProfileId: response.project.apiRouting.primaryProfileId,
                defaultModel: response.project.defaultModel,
              }
            : project,
        ),
      );
      setRunPreview(null);
      setRunError(null);
      if (response.configMirrorWarning) {
        setApiProfileError("设置已保存，但项目配置镜像暂时无法写入。");
      }
    } catch (error) {
      setApiProfileError(safeApiProfileError(error));
      throw error;
    }
  };

  const handleRunPreview = async (input: { prompt: string }) => {
    if (!selectedProjectId) return;
    setRunError(null);
    setRunPreparation(input);
    try {
      const response = await previewRun({
        projectId: selectedProjectId,
        ...input,
      });
      setRunPreview(response);
    } catch (error) {
      setRunError(safeRunError(error));
    }
  };

  const handleRunCreate = async () => {
    if (!selectedProjectId) return;
    setRunError(null);
    setIsCreatingRun(true);

    let createdRunId: string;
    try {
      const created = await createRun({
        projectId: selectedProjectId,
        ...runPreparation,
      });
      createdRunId = created.run.id;
      const project = projects.find((item) => item.id === selectedProjectId);
      setActiveRun({
        projectId: selectedProjectId,
        projectName: project?.name ?? "当前项目",
        status: "running",
      });
      setRunPreview(null);
    } catch (error) {
      setRunError(safeRunError(error));
      setActiveRun(null);
      setIsCreatingRun(false);
      return;
    }

    try {
      await executeRun({ runId: createdRunId });
      await refreshRunHistory(selectedProjectId, createdRunId);
    } catch (error) {
      // The Run is already persisted at this point. Keep the error wording
      // separate so an execution/persistence failure is not reported as a
      // failed creation.
      setRunError(safeRunExecutionError(error));
      await refreshRunHistory(selectedProjectId, createdRunId);
    } finally {
      setActiveRun(null);
      setIsCreatingRun(false);
    }
  };

  const handleLoadTaskDetail = async (taskId: string) => {
    if (!selectedProjectId) return;
    setRunResultsError(null);
    setSelectedTaskId(taskId);
    try {
      const detail = await getTask({
        projectId: selectedProjectId,
        taskId,
      });
      setTaskDetails((current) => ({ ...current, [taskId]: detail }));
    } catch (error) {
      setRunResultsError(safeRunResultsError(error));
    }
  };

  const handleReadResult = async (taskId: string) => {
    if (!selectedProjectId) return;
    setRunResultsError(null);
    try {
      const result = await readResult({
        projectId: selectedProjectId,
        taskId,
      });
      setResultPreview(result);
    } catch (error) {
      setRunResultsError(safeRunResultsError(error));
    }
  };

  return (
    <AppShell
      activeRun={activeRun}
      healthState={healthState}
      isAddingProject={isAddingProject}
      contextVersion={
        selectedProjectId ? (contexts[selectedProjectId] ?? null) : null
      }
      isGeneratingContext={isGeneratingContext}
      onAddProject={handleAddProject}
      onAuthorizeSensitiveFile={handleAuthorizeSensitiveFile}
      onGenerateContext={handleGenerateContext}
      onCancelScan={handleCancelScan}
      onAddTemporaryScanPattern={handleAddTemporaryScanPattern}
      onRemoveTemporaryScanPattern={handleRemoveTemporaryScanPattern}
      onSelectProject={setSelectedProjectId}
      onStartScan={handleStartScan}
      onRetryHealth={() => {
        setHealthState("checking");
        setRequestId((current) => current + 1);
      }}
      onSetFileIncluded={handleSetFileIncluded}
      onPreviewRun={handleRunPreview}
      onCreateRun={handleRunCreate}
      onCloseRunPreview={() => {
        setRunPreview(null);
        setRunError(null);
      }}
      runPreview={runPreview}
      runError={runError}
      runHistory={runHistory}
      runResultsError={runResultsError}
      runTasks={runTasks}
      selectedRunId={selectedRunId}
      selectedTaskId={selectedTaskId}
      taskDetails={taskDetails}
      isLoadingRunResults={isLoadingRunResults}
      onLoadTaskDetail={handleLoadTaskDetail}
      onOpenResult={handleReadResult}
      onSelectRun={setSelectedRunId}
      resultPreview={resultPreview}
      onCloseResultPreview={() => setResultPreview(null)}
      isCreatingRun={isCreatingRun}
      apiProfileError={apiProfileError}
      apiProfiles={apiProfiles}
      onDeleteApiProfile={handleDeleteApiProfile}
      onFetchApiModels={handleFetchApiModels}
      onPutApiProfileSecret={handlePutApiProfileSecret}
      onSaveApiProfile={handleSaveApiProfile}
      onTestApiProfile={handleTestApiProfile}
      onUpdateProjectRunSettings={handleUpdateProjectRunSettings}
      projectError={projectError}
      projects={projects}
      fileRecords={
        selectedProjectId ? (fileRecords[selectedProjectId] ?? []) : []
      }
      fileTotal={selectedProjectId ? (fileTotals[selectedProjectId] ?? 0) : 0}
      scanReport={
        selectedProjectId ? (scanReports[selectedProjectId] ?? null) : null
      }
      temporaryScanPatterns={
        selectedProjectId
          ? (temporaryScanPatterns[selectedProjectId] ?? [])
          : []
      }
      selectedProjectId={selectedProjectId}
    />
  );
}

function safeProjectError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      persistence_database_unavailable: "本地数据库暂不可用。",
      persistence_migration_failed: "本地数据库暂不可用。",
      persistence_transaction_failed: "项目数据暂时无法保存。",
      project_path_duplicate: "该目录已经登记，已保留原项目。",
      project_path_unavailable: "所选目录不可用。",
      scan_already_running: "当前项目已有扫描。",
      scan_cancelled: "扫描已取消，本次结果未提交。",
      scan_failed: "扫描失败，请检查项目路径和权限。",
      scan_not_found: "扫描操作不存在。",
      context_discovery_failed: "项目上下文文件无法读取。",
      security_sensitive_confirmation_required: "请先确认敏感文件授权。",
      security_sensitive_file_blocked: "敏感文件需要单独确认后才能纳入。",
      scan_file_unreadable: "文件不可读取，暂时不能纳入。",
      scan_encoding_unsupported: "文件编码不支持，暂时不能纳入。",
      scan_binary_file: "二进制文件不能纳入分析。",
      scan_file_too_large: "文件超过大小限制，暂时不能纳入。",
      validation_invalid_value: "文件状态无法修改。",
    };
    return messages[error.code] ?? "项目暂时无法添加。";
  }
  return "项目暂时无法添加。";
}

function safeApiProfileError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      api_profile_in_use: "该 API Profile 仍被项目使用，不能删除。",
      api_profile_name_duplicate: "API Profile 名称已存在。",
      provider_authentication_failed: "API Key 无效。",
      provider_connection_failed: "无法连接 API 服务。",
      provider_invalid_response: "API 服务返回格式无法识别。",
      provider_model_unavailable: "模型不可用。",
      provider_permission_denied: "API Profile 没有访问该模型的权限。",
      provider_rate_limited: "API 服务当前受到限流。",
      provider_server_error: "API 服务暂时不可用。",
      provider_timeout: "API 连接超时。",
      security_invalid_secret_reference: "API Profile 密钥引用无效。",
      security_secret_store_failure: "安全存储操作失败。",
      security_secret_store_unavailable: "安全存储不可用。",
      validation_invalid_value: "API Profile 配置无效。",
      validation_required_field: "请填写必填的 API Profile 信息。",
    };
    return messages[error.code] ?? "API Profile 操作失败。";
  }
  return "API Profile 操作失败。";
}

function safeRunError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      project_not_found: "项目不存在。",
      project_path_unavailable: "项目路径不可用。",
      run_active_exists: "当前已有活动 Run。",
      security_secret_not_found: "主 API Profile 尚未配置密钥。",
      validation_model_missing: "无法解析任务实际模型。",
      validation_required_field: "运行配置不完整或没有纳入文件。",
      validation_invalid_value: "目标文件尚未完成有效扫描。",
      persistence_database_unavailable: "本地数据库暂不可用。",
      persistence_transaction_failed: "Run 暂时无法创建。",
      run_not_found: "Run 不存在。",
      run_not_active: "Run 当前不可执行。",
      provider_connection_failed: "模型 Provider 暂不可用。",
      provider_timeout: "模型请求超时，Task 已记录为可重试失败。",
      provider_rate_limited: "模型服务限流，Task 已记录为可重试失败。",
      provider_server_error: "模型服务暂时异常，Task 已记录为可重试失败。",
      provider_authentication_failed: "模型认证失败，请检查 API Profile。",
      provider_permission_denied: "模型服务拒绝了当前请求。",
      provider_model_unavailable: "配置的模型不可用。",
      provider_invalid_request: "模型请求参数无效。",
      provider_invalid_response: "模型服务返回了无法识别的响应。",
      output_write_failed: "分析结果无法写入本地磁盘。",
    };
    return messages[error.code] ?? "Run 操作失败。";
  }
  return "Run 操作失败。";
}

function safeRunExecutionError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      persistence_database_unavailable: "Run 已创建，但本地数据库暂时不可用。",
      persistence_transaction_failed:
        "Run 已创建，但执行状态暂时无法保存，请刷新运行历史。",
      project_path_unavailable: "Run 已创建，但项目路径不可用。",
      run_not_found: "Run 已创建，但运行记录无法读取。",
      run_not_active: "Run 已创建，但当前状态不可执行。",
      provider_connection_failed: "Run 已创建，但模型 Provider 暂不可用。",
      provider_timeout: "模型请求超时，Task 已记录为可重试失败。",
      provider_rate_limited: "模型服务限流，Task 已记录为可重试失败。",
      provider_server_error: "模型服务暂时异常，Task 已记录为可重试失败。",
      provider_authentication_failed: "模型认证失败，请检查 API Profile。",
      provider_permission_denied: "模型服务拒绝了当前请求。",
      provider_model_unavailable: "配置的模型不可用。",
      provider_invalid_request: "模型请求参数无效。",
      provider_invalid_response: "模型服务返回了无法识别的响应。",
      output_write_failed: "分析结果无法写入本地磁盘。",
    };
    return messages[error.code] ?? "Run 已创建，但执行阶段失败。";
  }
  return "Run 已创建，但执行阶段失败。";
}

function safeRunResultsError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      persistence_database_unavailable: "运行数据暂时不可用。",
      persistence_transaction_failed: "运行数据暂时无法读取。",
      project_not_found: "项目不存在。",
      run_not_found: "Run 不存在或不属于当前项目。",
      task_not_found: "Task 不存在或不属于当前项目。",
      output_result_not_found: "当前 Task 没有可读取的结果。",
      output_result_too_large: "结果超过可预览大小。",
      output_result_read_failed: "结果文件暂时无法读取。",
      security_path_escape: "结果路径无效，已阻止读取。",
      validation_invalid_value: "运行列表分页参数无效。",
      validation_limit_exceeded: "运行列表分页大小无效。",
    };
    return messages[error.code] ?? "运行结果暂时无法读取。";
  }
  return "运行结果暂时无法读取。";
}

function isIpcError(error: unknown): error is IpcError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    "message" in error &&
    typeof error.message === "string"
  );
}

function toShellProject(project: ProjectSummaryDto): ShellProject {
  return project;
}
