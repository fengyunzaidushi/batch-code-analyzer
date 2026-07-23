import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ApiProfileSaveRequest,
  ApiProfileSummaryDto,
  ResultReadResponse,
  ContextVersionDto,
  FileRecordSummaryDto,
  IpcError,
  ProjectDetailDto,
  ProjectPromptSaveRequest,
  ProjectPromptSelectRequest,
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
  saveProjectPrompt as savePromptPreset,
  selectProjectPrompt as selectPromptPreset,
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
  getApiProfileSecret,
  listApiProfiles,
  putApiProfileSecret,
  saveApiProfile,
  testApiProfile,
  type ApiProfileSecretPutRequest,
} from "../ipc/apiProfiles";
import {
  cancelRun,
  createRun,
  executeRun,
  getTask,
  listRuns,
  listTasks,
  previewRun,
  readResult,
  retryTask,
  retryTasks,
} from "../ipc/runs";
import { generatePrompt } from "../ipc/prompt";
import { AppShell, type ShellHealthState, type ShellProject } from "./AppShell";

interface RetryQueueItem {
  projectId: string;
  projectName: string;
  runId: string;
  taskId: string;
}

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
    runId?: string;
    projectId: string;
    projectName: string;
    status: "running" | "cancelling" | "interrupted";
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
  const [retryingTaskIds, setRetryingTaskIds] = useState<string[]>([]);
  const [isBatchRetrying, setIsBatchRetrying] = useState(false);
  const [batchRetryTargetCount, setBatchRetryTargetCount] = useState(0);
  const retryQueue = useRef<RetryQueueItem[]>([]);
  const retryingTaskIdSet = useRef(new Set<string>());
  const retryQueueRunning = useRef(false);
  const retryQueueRunId = useRef<string | null>(null);
  const activeRetryTaskId = useRef<string | null>(null);
  const batchRetryRunning = useRef(false);

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

  const refreshRunTasks = useCallback(
    async (projectId: string, runId: string) => {
      const response = await listTasks({ projectId, runId, limit: 500 });
      setRunTasks(response.items);
      setRunResultsError(null);
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
                  concurrency: detail.concurrency,
                  defaultPrompt: detail.defaultPrompt,
                  promptPresets: detail.promptPresets,
                  activePromptId: detail.activePromptId,
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
        const activeHistoryRun = response.items.find((run) =>
          ["running", "pausing", "paused", "cancelling"].includes(run.status),
        );
        if (activeHistoryRun) {
          const project = projects.find(
            (item) => item.id === selectedProjectId,
          );
          setActiveRun({
            runId: activeHistoryRun.id,
            projectId: selectedProjectId,
            projectName: project?.name ?? "当前项目",
            status:
              activeHistoryRun.status === "cancelling"
                ? "cancelling"
                : "running",
          });
        } else {
          setActiveRun((current) =>
            current?.projectId === selectedProjectId ? null : current,
          );
        }
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
        setActiveRun((current) =>
          current?.projectId === selectedProjectId ? null : current,
        );
      },
    );
    return () => {
      active = false;
    };
  }, [projects, selectedProjectId]);

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

  const handleGetApiProfileSecret = async (
    profileId: string,
  ): Promise<string> => {
    setApiProfileError(null);
    try {
      const response = await getApiProfileSecret({ profileId });
      return response.secret;
    } catch (error) {
      const message = safeApiProfileError(error);
      setApiProfileError(message);
      throw new Error(message, { cause: error });
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
                concurrency: response.project.concurrency,
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

  const applyProjectDetail = (detail: ProjectDetailDto) => {
    setProjects((current) =>
      current.map((project) =>
        project.id === detail.id
          ? {
              ...project,
              rootDirectory: detail.sourceDirectory,
              primaryProfileId: detail.apiRouting.primaryProfileId,
              defaultModel: detail.defaultModel,
              defaultPrompt: detail.defaultPrompt,
              promptPresets: detail.promptPresets,
              activePromptId: detail.activePromptId,
            }
          : project,
      ),
    );
  };

  const handleSaveProjectPrompt = async (
    request: ProjectPromptSaveRequest,
  ): Promise<void> => {
    try {
      const response = await savePromptPreset(request);
      applyProjectDetail(response.project);
      if (response.configMirrorWarning) {
        setProjectError("提示词已保存，但项目配置镜像暂时无法写入。");
      } else {
        setProjectError(null);
      }
    } catch (error) {
      setProjectError(safeProjectError(error));
      throw new Error(safeProjectError(error), { cause: error });
    }
  };

  const handleSelectProjectPrompt = async (
    request: ProjectPromptSelectRequest,
  ): Promise<void> => {
    try {
      const response = await selectPromptPreset(request);
      applyProjectDetail(response.project);
      if (response.configMirrorWarning) {
        setProjectError("提示词已切换，但项目配置镜像暂时无法写入。");
      } else {
        setProjectError(null);
      }
    } catch (error) {
      setProjectError(safeProjectError(error));
      throw new Error(safeProjectError(error), { cause: error });
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

  const handleGeneratePrompt = async (goal: string): Promise<string> => {
    if (!selectedProjectId) throw new Error("请先选择项目");
    try {
      const response = await generatePrompt({
        projectId: selectedProjectId,
        goal,
      });
      return response.prompt;
    } catch (error) {
      throw new Error(safePromptGenerationError(error), { cause: error });
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
        runId: createdRunId,
        projectId: selectedProjectId,
        projectName: project?.name ?? "当前项目",
        status: "running",
      });
      setRunPreview(null);
      await refreshRunHistory(selectedProjectId, createdRunId);
    } catch (error) {
      setRunError(safeRunError(error));
      setActiveRun(null);
      setIsCreatingRun(false);
      return;
    }

    const refreshTimer = window.setInterval(() => {
      void refreshRunTasks(selectedProjectId, createdRunId).catch((error) => {
        setRunResultsError(safeRunResultsError(error));
      });
    }, 1000);
    try {
      await executeRun({ runId: createdRunId });
      await refreshRunHistory(selectedProjectId, createdRunId);
      await refreshRunTasks(selectedProjectId, createdRunId);
    } catch (error) {
      // The Run is already persisted at this point. Keep the error wording
      // separate so an execution/persistence failure is not reported as a
      // failed creation.
      setRunError(safeRunExecutionError(error));
      await refreshRunHistory(selectedProjectId, createdRunId);
      await refreshRunTasks(selectedProjectId, createdRunId);
    } finally {
      window.clearInterval(refreshTimer);
      setActiveRun(null);
      setIsCreatingRun(false);
    }
  };

  const handleRunCancel = async () => {
    const current = activeRun;
    if (!current?.runId) return;
    retryQueue.current = [];
    if (retryQueueRunning.current) {
      const currentTaskId = activeRetryTaskId.current;
      retryingTaskIdSet.current = new Set(currentTaskId ? [currentTaskId] : []);
      setRetryingTaskIds(currentTaskId ? [currentTaskId] : []);
    }
    setRunError(null);
    setActiveRun({ ...current, status: "cancelling" });
    try {
      await cancelRun(current.runId);
      setActiveRun(null);
      await refreshRunHistory(current.projectId, current.runId);
      if (selectedProjectId === current.projectId) {
        await refreshRunTasks(current.projectId, current.runId);
      }
    } catch (error) {
      setRunError(safeRunCancellationError(error));
      await refreshRunHistory(current.projectId, current.runId).catch(() => {
        // Keep the original cancellation error visible when history refresh fails.
      });
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

  const drainRetryQueue = useCallback(async () => {
    if (retryQueueRunning.current) return;
    retryQueueRunning.current = true;
    while (retryQueue.current.length > 0) {
      const item = retryQueue.current.shift();
      if (!item) break;
      activeRetryTaskId.current = item.taskId;
      setRunResultsError(null);
      setActiveRun({
        runId: item.runId,
        projectId: item.projectId,
        projectName: item.projectName,
        status: "running",
      });
      const refreshTimer = window.setInterval(() => {
        void refreshRunTasks(item.projectId, item.runId).catch((error) => {
          setRunResultsError(safeRunResultsError(error));
        });
      }, 1000);
      try {
        await retryTask({ projectId: item.projectId, taskId: item.taskId });
        await refreshRunHistory(item.projectId, item.runId);
        await refreshRunTasks(item.projectId, item.runId);
        const detail = await getTask({
          projectId: item.projectId,
          taskId: item.taskId,
        });
        setTaskDetails((current) => ({
          ...current,
          [item.taskId]: detail,
        }));
        setSelectedTaskId(item.taskId);
      } catch (error) {
        const retryError = safeRunResultsError(error);
        await refreshRunHistory(item.projectId, item.runId).catch(() => {
          // Preserve the retry error when refreshing persisted state also fails.
        });
        await refreshRunTasks(item.projectId, item.runId).catch(() => {
          // Preserve the retry error when refreshing persisted state also fails.
        });
        setRunResultsError(retryError);
      } finally {
        window.clearInterval(refreshTimer);
        activeRetryTaskId.current = null;
        retryingTaskIdSet.current.delete(item.taskId);
        setRetryingTaskIds(Array.from(retryingTaskIdSet.current));
      }
    }
    const completedRunId = retryQueueRunId.current;
    retryQueueRunning.current = false;
    retryQueueRunId.current = null;
    setActiveRun((current) =>
      current?.runId === completedRunId ? null : current,
    );
  }, [refreshRunHistory, refreshRunTasks]);

  const handleRetryTask = async (taskId: string) => {
    if (!selectedProjectId || batchRetryRunning.current) return;
    const task = runTasks.find((item) => item.id === taskId);
    if (!task || task.status !== "failed") return;
    if (retryingTaskIdSet.current.has(taskId)) return;
    if (
      retryQueueRunId.current !== null &&
      retryQueueRunId.current !== task.runId
    ) {
      return;
    }
    if (
      activeRun &&
      (activeRun.status !== "running" || retryQueueRunId.current !== task.runId)
    ) {
      return;
    }
    const project = projects.find((item) => item.id === selectedProjectId);
    retryQueueRunId.current = task.runId;
    retryQueue.current.push({
      projectId: selectedProjectId,
      projectName: project?.name ?? "当前项目",
      runId: task.runId,
      taskId,
    });
    retryingTaskIdSet.current.add(taskId);
    setRetryingTaskIds(Array.from(retryingTaskIdSet.current));
    await drainRetryQueue();
  };

  const handleRetryTasks = async (taskIds: readonly string[]) => {
    if (
      !selectedProjectId ||
      !selectedRunId ||
      taskIds.length === 0 ||
      activeRun ||
      retryQueueRunning.current ||
      batchRetryRunning.current
    ) {
      return;
    }
    const project = projects.find((item) => item.id === selectedProjectId);
    batchRetryRunning.current = true;
    setIsBatchRetrying(true);
    setBatchRetryTargetCount(taskIds.length);
    setRunResultsError(null);
    setActiveRun({
      runId: selectedRunId,
      projectId: selectedProjectId,
      projectName: project?.name ?? "当前项目",
      status: "running",
    });
    const refreshTimer = window.setInterval(() => {
      void refreshRunTasks(selectedProjectId, selectedRunId).catch((error) => {
        setRunResultsError(safeRunResultsError(error));
      });
    }, 1000);
    try {
      await retryTasks({
        projectId: selectedProjectId,
        runId: selectedRunId,
        taskIds: [...taskIds],
      });
      await refreshRunHistory(selectedProjectId, selectedRunId);
      await refreshRunTasks(selectedProjectId, selectedRunId);
    } catch (error) {
      const retryError = safeRunResultsError(error);
      await refreshRunHistory(selectedProjectId, selectedRunId).catch(() => {
        // Preserve the retry error when refreshing persisted state also fails.
      });
      await refreshRunTasks(selectedProjectId, selectedRunId).catch(() => {
        // Preserve the retry error when refreshing persisted state also fails.
      });
      setRunResultsError(retryError);
    } finally {
      window.clearInterval(refreshTimer);
      batchRetryRunning.current = false;
      setIsBatchRetrying(false);
      setBatchRetryTargetCount(0);
      setActiveRun((current) =>
        current?.runId === selectedRunId ? null : current,
      );
    }
  };

  return (
    <AppShell
      activeRun={activeRun}
      onCancelRun={() => void handleRunCancel()}
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
      onGeneratePrompt={handleGeneratePrompt}
      onSaveProjectPrompt={handleSaveProjectPrompt}
      onSelectProjectPrompt={handleSelectProjectPrompt}
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
      onRetryTask={handleRetryTask}
      onRetryTasks={handleRetryTasks}
      onSelectRun={setSelectedRunId}
      retryingTaskIds={retryingTaskIds}
      isBatchRetrying={isBatchRetrying}
      batchRetryTargetCount={batchRetryTargetCount}
      resultPreview={resultPreview}
      onCloseResultPreview={() => setResultPreview(null)}
      isCreatingRun={isCreatingRun}
      apiProfileError={apiProfileError}
      apiProfiles={apiProfiles}
      onDeleteApiProfile={handleDeleteApiProfile}
      onFetchApiModels={handleFetchApiModels}
      onGetApiProfileSecret={handleGetApiProfileSecret}
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
      prompt_not_found: "保存的提示词不存在。",
      validation_required_field: "提示词名称和内容不能为空。",
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
      security_secret_not_found: "API Key 不存在，请重新配置。",
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

function safePromptGenerationError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      validation_required_field: "请先描述这次分析希望回答的问题。",
      validation_api_profile_missing: "尚未配置主 API Profile。",
      validation_model_missing: "请先配置项目默认模型。",
      security_secret_not_found: "API Key 当前不可用，请重新配置。",
      provider_connection_failed: "无法连接模型服务。",
      provider_timeout: "提示词生成超时，请重试。",
      provider_rate_limited: "模型服务当前受到限流，请稍后重试。",
      provider_server_error: "模型服务暂时异常，请稍后重试。",
      provider_invalid_response: "模型没有返回有效的提示词。",
      persistence_database_unavailable: "本地数据库暂不可用。",
      persistence_transaction_failed: "项目数据暂时无法读取。",
    };
    return messages[error.code] ?? "提示词生成失败，请重试。";
  }
  return error instanceof Error ? error.message : "提示词生成失败，请重试。";
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
      security_secret_not_found:
        "Run 已创建，但当前进程无法读取 API Key，请重新配置。",
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

function safeRunCancellationError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      persistence_database_unavailable: "运行数据暂时不可用。",
      persistence_transaction_failed: "取消状态暂时无法保存，请重试。",
      run_not_found: "Run 不存在。",
      run_not_active: "Run 已经结束，正在刷新运行历史。",
    };
    return messages[error.code] ?? "Run 暂时无法取消。";
  }
  return "Run 暂时无法取消。";
}

function safeRunResultsError(error: unknown): string {
  if (isIpcError(error)) {
    const messages: Record<string, string> = {
      persistence_database_unavailable: "运行数据暂时不可用。",
      persistence_transaction_failed: "运行数据暂时无法读取。",
      project_not_found: "项目不存在。",
      run_not_found: "Run 不存在或不属于当前项目。",
      task_not_found: "Task 不存在或不属于当前项目。",
      task_cannot_retry: "当前失败不支持重试，请查看尝试详情。",
      run_active_exists: "已有其他 Run 正在执行，暂时不能重试。",
      run_not_active: "原 Run 当前不能重新执行。",
      security_secret_not_found: "原 Run 使用的 API Profile 密钥不可用。",
      provider_connection_failed: "模型 Provider 暂不可用。",
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
