import { useCallback, useEffect, useState } from "react";
import type {
  IpcError,
  ProjectSummaryDto,
} from "@batch-code-analyzer/ipc-types";

import {
  addProject,
  chooseProjectDirectory,
  getProject,
  listProjects,
} from "../ipc/projects";
import { checkBackendHealth } from "../ipc/health";
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

  const refreshProjects = useCallback(async () => {
    const items = await listProjects();
    setProjects(items.map(toShellProject));
    setSelectedProjectId((current) =>
      items.some((item) => item.id === current)
        ? current
        : (items[0]?.id ?? null),
    );
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

  return (
    <AppShell
      healthState={healthState}
      isAddingProject={isAddingProject}
      onAddProject={handleAddProject}
      onSelectProject={setSelectedProjectId}
      onRetryHealth={() => {
        setHealthState("checking");
        setRequestId((current) => current + 1);
      }}
      projectError={projectError}
      projects={projects}
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
