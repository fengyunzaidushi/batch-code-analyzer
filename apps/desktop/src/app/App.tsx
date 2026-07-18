import { useCallback, useEffect, useState } from "react";
import type {
  FileRecordSummaryDto,
  IpcError,
  ProjectSummaryDto,
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

  return (
    <AppShell
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
