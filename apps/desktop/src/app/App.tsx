import { useEffect, useState } from "react";
import type { ProjectSummaryDto } from "@batch-code-analyzer/ipc-types";

import { listProjects } from "../ipc/projects";
import { checkBackendHealth } from "../ipc/health";
import { AppShell, type ShellHealthState, type ShellProject } from "./AppShell";

export function App() {
  const [healthState, setHealthState] = useState<ShellHealthState>("checking");
  const [requestId, setRequestId] = useState(0);
  const [projects, setProjects] = useState<ShellProject[]>([]);

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
        if (active) setProjects(items.map(toShellProject));
      },
      () => {
        if (active) setProjects([]);
      },
    );
    return () => {
      active = false;
    };
  }, []);

  return (
    <AppShell
      healthState={healthState}
      onAddProject={() => undefined}
      onRetryHealth={() => {
        setHealthState("checking");
        setRequestId((current) => current + 1);
      }}
      projects={projects}
    />
  );
}

function toShellProject(project: ProjectSummaryDto): ShellProject {
  return project;
}
