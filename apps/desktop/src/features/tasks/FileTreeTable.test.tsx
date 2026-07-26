import { render, screen, within } from "@testing-library/react";
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
    render(<FileTreeTable files={[file({ sizeBytes: 20_002 })]} />);

    const warning = screen.getByText("约 10,001 tokens · 代码文件过长");
    expect(warning).toBeInTheDocument();
    expect(warning.parentElement).toHaveClass("is-warning");
  });

  it("does not warn at the threshold but warns when a long file is excluded", () => {
    render(
      <FileTreeTable
        files={[
          file({ id: "threshold", sizeBytes: 20_000 }),
          file({
            exclusionReason: "user_excluded",
            id: "excluded",
            included: false,
            relativePath: "src/large.ts",
            sizeBytes: 40_000,
          }),
        ]}
      />,
    );

    expect(screen.getByText("约 10,000 tokens")).toBeInTheDocument();
    const warning = screen.getByText("约 20,000 tokens · 代码文件过长");
    expect(warning).toBeInTheDocument();
    expect(warning.parentElement).toHaveClass("is-warning");
  });

  it("sorts files by estimated tokens in both directions", async () => {
    const user = userEvent.setup();
    render(
      <FileTreeTable
        files={[
          file({ id: "large", relativePath: "src/large.ts", sizeBytes: 600 }),
          file({ id: "small", relativePath: "src/small.ts", sizeBytes: 20 }),
          file({ id: "medium", relativePath: "src/medium.ts", sizeBytes: 200 }),
        ]}
      />,
    );

    const table = screen.getByRole("table", { name: "扫描文件列表" });
    await user.click(
      screen.getByRole("button", {
        name: "按文件列表大小 / 预估 Token升序排序",
      }),
    );
    expect(
      within(table)
        .getAllByTitle(/src\/(small|medium|large)\.ts/)
        .map((element) => element.getAttribute("title")),
    ).toEqual(["src/small.ts", "src/medium.ts", "src/large.ts"]);
    expect(
      screen.getByRole("button", {
        name: "按文件列表大小 / 预估 Token降序排序",
      }).parentElement,
    ).toHaveAttribute("aria-sort", "ascending");

    await user.click(
      screen.getByRole("button", {
        name: "按文件列表大小 / 预估 Token降序排序",
      }),
    );
    expect(
      within(table)
        .getAllByTitle(/src\/(small|medium|large)\.ts/)
        .map((element) => element.getAttribute("title")),
    ).toEqual(["src/large.ts", "src/medium.ts", "src/small.ts"]);
  });

  it("groups sensitive files together when sorting by status", async () => {
    const user = userEvent.setup();
    render(
      <FileTreeTable
        files={[
          file({ id: "normal", relativePath: "src/normal.ts" }),
          file({
            id: "secret-a",
            relativePath: "config/a.env",
            included: false,
            exclusionReason: "sensitive_filename",
            sourceStatus: "sensitive",
          }),
          file({
            id: "secret-b",
            relativePath: "config/b.env",
            included: false,
            exclusionReason: "sensitive_content",
            sourceStatus: "sensitive",
          }),
        ]}
      />,
    );

    const table = screen.getByRole("table", { name: "扫描文件列表" });
    await user.click(
      screen.getByRole("button", { name: "按文件列表状态升序排序" }),
    );
    expect(
      within(table)
        .getAllByTitle(/(config|src)\//)
        .map((element) => element.getAttribute("title")),
    ).toEqual(["config/a.env", "config/b.env", "src/normal.ts"]);
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
