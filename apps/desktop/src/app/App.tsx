import { useCallback, useEffect, useState } from "react";
import type {
  ApiProfileSaveRequest,
  ApiProfileSummaryDto,
  FileRecordSummaryDto,
  IpcError,
  ProjectSummaryDto,
  RunPreviewRequest,
  RunPreviewResponse,
  ScanReportDto,
} from "@batch-code-analyzer/ipc-types";

import {
  addProject,
  chooseProjectDirectory,
  getProject,
  listProjects,
} from "../ipc/projects";
import { checkBackendHealth } from "../ipc/health";
import { listFiles, setFileIncluded } from "../ipc/files";
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
import { createRun, previewRun } from "../ipc/runs";
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
  const [fileRecords, setFileRecords] = useState<
    Record<string, FileRecordSummaryDto[]>
  >({});
  const [fileTotals, setFileTotals] = useState<Record<string, number>>({});
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
              ? { ...project, rootDirectory: detail.sourceDirectory }
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

  const handleStartScan = async () => {
    if (!selectedProjectId) return;
    setProjectError(null);
    try {
      const response = await startScan(selectedProjectId);
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
    try {
      await createRun({ projectId: selectedProjectId, ...runPreparation });
      const project = projects.find((item) => item.id === selectedProjectId);
      setActiveRun({
        projectId: selectedProjectId,
        projectName: project?.name ?? "当前项目",
        status: "running",
      });
      setRunPreview(null);
    } catch (error) {
      setRunError(safeRunError(error));
    } finally {
      setIsCreatingRun(false);
    }
  };

  return (
    <AppShell
      activeRun={activeRun}
      healthState={healthState}
      isAddingProject={isAddingProject}
      onAddProject={handleAddProject}
      onCancelScan={handleCancelScan}
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
      isCreatingRun={isCreatingRun}
      apiProfileError={apiProfileError}
      apiProfiles={apiProfiles}
      onDeleteApiProfile={handleDeleteApiProfile}
      onFetchApiModels={handleFetchApiModels}
      onPutApiProfileSecret={handlePutApiProfileSecret}
      onSaveApiProfile={handleSaveApiProfile}
      onTestApiProfile={handleTestApiProfile}
      projectError={projectError}
      projects={projects}
      fileRecords={
        selectedProjectId ? (fileRecords[selectedProjectId] ?? []) : []
      }
      fileTotal={selectedProjectId ? (fileTotals[selectedProjectId] ?? 0) : 0}
      scanReport={
        selectedProjectId ? (scanReports[selectedProjectId] ?? null) : null
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
    };
    return messages[error.code] ?? "Run 操作失败。";
  }
  return "Run 操作失败。";
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
