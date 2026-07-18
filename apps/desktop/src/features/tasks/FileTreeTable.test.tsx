import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { FileRecordSummaryDto } from "@batch-code-analyzer/ipc-types";

import { FileTreeTable } from "./FileTreeTable";

function file(
  overrides: Partial<FileRecordSummaryDto> = {},
): FileRecordSummaryDto {
  return {
    exclusionReason: null,
    id: "file-1",
    included: true,
    language: "typescript",
    modifiedAt: "2026-07-18T12:00:00Z",
    projectId: "project-1",
    relativePath: "src/main.ts",
    resultStatus: "none",
    schemaVersion: 1,
    sizeBytes: 42,
    sourceStatus: "normal",
    ...overrides,
  };
}

describe("FileTreeTable", () => {
  it("groups files into an expanded directory tree", () => {
    render(
      <FileTreeTable
        files={[
          file({ id: "file-main", relativePath: "src/main.ts" }),
          file({ id: "file-util", relativePath: "src/lib/util.ts" }),
          file({ id: "file-readme", relativePath: "README.md" }),
        ]}
      />,
    );

    expect(screen.getByText("src")).toBeInTheDocument();
    expect(screen.getByText("lib")).toBeInTheDocument();
    expect(screen.getByText("main.ts")).toHaveAttribute("title", "src/main.ts");
    expect(screen.getByText("util.ts")).toHaveAttribute(
      "title",
      "src/lib/util.ts",
    );
    expect(screen.getByText("README.md")).toHaveAttribute("title", "README.md");
  });

  it("collapses a directory without losing its tree node", async () => {
    const user = userEvent.setup();
    render(
      <FileTreeTable
        files={[
          file({ id: "file-main", relativePath: "src/main.ts" }),
          file({ id: "file-util", relativePath: "src/lib/util.ts" }),
        ]}
      />,
    );

    await user.click(screen.getByRole("button", { name: "折叠 src" }));

    expect(screen.getByText("src")).toBeInTheDocument();
    expect(screen.queryByText("main.ts")).not.toBeInTheDocument();
    expect(screen.queryByText("lib")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "展开 src" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("uses a readable label for sensitive files", () => {
    render(
      <FileTreeTable
        files={[
          file({
            exclusionReason: "sensitive",
            id: "file-env",
            included: false,
            relativePath: ".env",
            sourceStatus: "sensitive",
          }),
        ]}
      />,
    );

    expect(screen.getByText("已排除：敏感文件")).toBeInTheDocument();
    expect(screen.queryByText("已排除：sensitive")).not.toBeInTheDocument();
  });
});
