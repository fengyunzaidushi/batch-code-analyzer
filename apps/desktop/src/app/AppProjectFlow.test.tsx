import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  addProject: vi.fn(),
  checkBackendHealth: vi.fn(),
  chooseProjectDirectory: vi.fn(),
  getProject: vi.fn(),
  listProjects: vi.fn(),
}));

vi.mock("../ipc/health", () => ({
  checkBackendHealth: mocks.checkBackendHealth,
}));
vi.mock("../ipc/projects", () => ({
  addProject: mocks.addProject,
  chooseProjectDirectory: mocks.chooseProjectDirectory,
  getProject: mocks.getProject,
  listProjects: mocks.listProjects,
}));

import { App } from "./App";

describe("App project flow", () => {
  afterEach(() => {
    vi.resetAllMocks();
  });

  it("does nothing when the native directory picker is cancelled", async () => {
    const user = userEvent.setup();
    arrangeEmptyProject();
    mocks.chooseProjectDirectory.mockResolvedValue(null);
    render(<App />);

    await user.click(screen.getAllByRole("button", { name: "添加项目" })[1]!);
    await waitFor(() =>
      expect(mocks.chooseProjectDirectory).toHaveBeenCalledOnce(),
    );
    expect(mocks.addProject).not.toHaveBeenCalled();
  });

  it("refreshes and selects the project returned by project_add", async () => {
    const user = userEvent.setup();
    const detail = projectDetail();
    arrangeEmptyProject();
    mocks.chooseProjectDirectory.mockResolvedValue("/workspace/demo");
    mocks.addProject.mockResolvedValue({
      configMirrorWarning: false,
      created: true,
      project: detail,
    });
    mocks.listProjects.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        id: detail.id,
        lastOpenedAt: detail.lastOpenedAt,
        name: detail.name,
        pathStatus: detail.pathStatus,
        schemaVersion: 1,
      },
    ]);
    mocks.getProject.mockResolvedValue(detail);
    render(<App />);

    await user.click(screen.getAllByRole("button", { name: "添加项目" })[1]!);
    expect(await screen.findAllByText("Demo")).toHaveLength(2);
    expect(screen.getAllByText("/workspace/demo")).toHaveLength(2);
    expect(mocks.addProject).toHaveBeenCalledWith({
      sourceDirectory: "/workspace/demo",
    });
  });

  it("locates an existing project when project_add reports a duplicate", async () => {
    const user = userEvent.setup();
    const detail = projectDetail();
    arrangeEmptyProject();
    mocks.chooseProjectDirectory.mockResolvedValue("/workspace/demo");
    mocks.addProject.mockResolvedValue({
      configMirrorWarning: false,
      created: false,
      project: detail,
    });
    mocks.listProjects.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        id: detail.id,
        lastOpenedAt: detail.lastOpenedAt,
        name: detail.name,
        pathStatus: detail.pathStatus,
        schemaVersion: 1,
      },
    ]);
    mocks.getProject.mockResolvedValue(detail);
    render(<App />);

    await user.click(screen.getAllByRole("button", { name: "添加项目" })[1]!);
    const projectItem = await within(screen.getByRole("list")).findByText(
      "Demo",
    );
    expect(projectItem.closest("button")).toHaveClass("is-selected");
  });

  it("shows a safe message for a project command error", async () => {
    const user = userEvent.setup();
    arrangeEmptyProject();
    mocks.chooseProjectDirectory.mockResolvedValue("/missing");
    mocks.addProject.mockRejectedValue({
      code: "project_path_unavailable",
      message: "internal path details must not reach UI",
    });
    render(<App />);

    await user.click(screen.getAllByRole("button", { name: "添加项目" })[1]!);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "所选目录不可用",
    );
    expect(
      screen.queryByText("internal path details must not reach UI"),
    ).not.toBeInTheDocument();
  });
});

function arrangeEmptyProject() {
  mocks.checkBackendHealth.mockResolvedValue({
    appVersion: "0.1.0",
    databaseSchemaVersion: 1,
    databaseStatus: "ready",
    schemaVersion: 1,
    status: "ready",
  });
  mocks.listProjects.mockResolvedValue([]);
}

function projectDetail() {
  return {
    apiRouting: { fallbacks: [], primaryProfileId: null },
    contextModel: null,
    defaultModel: null,
    defaultPrompt: "prompt",
    id: "project-demo",
    lastOpenedAt: "2026-07-18T12:00:00Z",
    name: "Demo",
    outputRoot: null,
    pathStatus: "available" as const,
    schemaVersion: 1 as const,
    sourceDirectory: "/workspace/demo",
  };
}
