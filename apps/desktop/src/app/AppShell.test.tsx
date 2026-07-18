import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ProjectSummaryDto } from "@batch-code-analyzer/ipc-types";

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
});

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
