import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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

  it("shows file size and a conservative token estimate", () => {
    render(<FileTreeTable files={[file({ sizeBytes: 2048 })]} />);

    expect(screen.getByText("大小 / 预估 Token")).toBeInTheDocument();
    expect(screen.getByText("2 KB")).toBeInTheDocument();
    expect(screen.getByText("约 1,024 tokens")).toBeInTheDocument();
  });

  it("warns when an included file exceeds the token threshold", () => {
    render(<FileTreeTable files={[file({ sizeBytes: 64_002 })]} />);

    expect(
      screen.getByText("约 32,001 tokens · 代码文件过长"),
    ).toBeInTheDocument();
  });

  it("does not warn at the threshold or when a long file is excluded", () => {
    render(
      <FileTreeTable
        files={[
          file({ id: "threshold", sizeBytes: 64_000 }),
          file({
            exclusionReason: "user_excluded",
            id: "excluded",
            included: false,
            relativePath: "src/large.ts",
            sizeBytes: 128_000,
          }),
        ]}
      />,
    );

    expect(screen.queryByText(/代码文件过长/)).not.toBeInTheDocument();
    expect(screen.getByText("约 32,000 tokens")).toBeInTheDocument();
    expect(screen.getByText("约 64,000 tokens")).toBeInTheDocument();
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

  it("delegates a normal file inclusion toggle", async () => {
    const user = userEvent.setup();
    const onSetIncluded = vi.fn().mockResolvedValue(undefined);
    render(<FileTreeTable files={[file()]} onSetIncluded={onSetIncluded} />);

    await user.click(
      screen.getByRole("checkbox", { name: "排除文件 src/main.ts" }),
    );

    expect(onSetIncluded).toHaveBeenCalledWith(
      expect.objectContaining({ relativePath: "src/main.ts" }),
      false,
    );
  });

  it("disables inclusion for sensitive files", () => {
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
        onSetIncluded={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: "纳入文件 .env" }),
    ).toBeDisabled();
  });

  it("requires explicit confirmation before authorizing a sensitive file", async () => {
    const user = userEvent.setup();
    const onAuthorizeSensitive = vi.fn().mockResolvedValue(undefined);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(
      <FileTreeTable
        files={[
          file({
            exclusionReason: "sensitive_content",
            id: "file-secret",
            included: false,
            relativePath: "src/config.ts",
            sourceStatus: "sensitive",
          }),
        ]}
        onAuthorizeSensitive={onAuthorizeSensitive}
        onSetIncluded={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(
      screen.getByRole("button", { name: "授权并纳入文件 src/config.ts" }),
    );

    expect(confirm).toHaveBeenCalled();
    expect(onAuthorizeSensitive).toHaveBeenCalledWith("file-secret");
    confirm.mockRestore();
  });

  it("shows an authorized sensitive file as ready for processing", () => {
    render(
      <FileTreeTable
        files={[
          file({
            exclusionReason: "user_authorized_sensitive",
            included: true,
            relativePath: "src/config.ts",
            sourceStatus: "sensitive",
          }),
        ]}
      />,
    );

    expect(screen.getByText("已授权，待处理")).toBeInTheDocument();
  });
});
