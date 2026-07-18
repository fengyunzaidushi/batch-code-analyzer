import {
  ChevronDown,
  ChevronRight,
  FileCode2,
  Folder,
  FolderOpen,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { FileRecordSummaryDto } from "@batch-code-analyzer/ipc-types";

import { VirtualTaskTable } from "./VirtualTaskTable";

interface FileTreeTableProps {
  files: readonly FileRecordSummaryDto[];
  emptyLabel?: string;
}

interface DirectoryNode {
  kind: "directory";
  key: string;
  name: string;
  depth: number;
  directories: DirectoryNode[];
  files: FileNode[];
  fileCount: number;
}

interface FileNode {
  kind: "file";
  key: string;
  name: string;
  depth: number;
  file: FileRecordSummaryDto;
}

type TreeRow = DirectoryNode | FileNode;

interface MutableDirectory extends DirectoryNode {
  directoryMap: Map<string, MutableDirectory>;
}

const EXCLUSION_LABELS: Record<string, string> = {
  binary: "二进制文件",
  builtin_extension: "不支持的文件类型",
  file_too_large: "文件过大",
  gitignore_or_user_pattern: "被忽略规则排除",
  not_included_extension: "不在纳入扩展名内",
  sensitive: "敏感文件",
  sensitive_content: "检测到敏感内容",
  sensitive_filename: "敏感文件名",
  symlink: "符号链接",
  unreadable: "无法读取",
  unsupported_encoding: "编码不支持",
};

export function FileTreeTable({
  files,
  emptyLabel = "暂无文件",
}: FileTreeTableProps) {
  const [collapsedDirectories, setCollapsedDirectories] = useState<Set<string>>(
    () => new Set(),
  );
  const root = useMemo(() => buildFileTree(files), [files]);
  const rows = useMemo(
    () => flattenFileTree(root, collapsedDirectories),
    [collapsedDirectories, root],
  );

  const toggleDirectory = (key: string) => {
    setCollapsedDirectories((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  return (
    <VirtualTaskTable
      emptyLabel={emptyLabel}
      getRowKey={(row) => row.key}
      header={
        <>
          <span>文件</span>
          <span>状态</span>
          <span>模型</span>
          <span>结果</span>
        </>
      }
      items={rows}
      renderRow={(row) => {
        if (row.kind === "directory") {
          const isCollapsed = collapsedDirectories.has(row.key);
          return (
            <>
              <span
                className="file-tree-cell file-tree-directory"
                style={{ paddingLeft: `${row.depth * 18}px` }}
              >
                <button
                  aria-label={`${isCollapsed ? "展开" : "折叠"} ${row.name}`}
                  aria-expanded={!isCollapsed}
                  className="file-tree-toggle"
                  onClick={() => toggleDirectory(row.key)}
                  title={isCollapsed ? `展开 ${row.name}` : `折叠 ${row.name}`}
                  type="button"
                >
                  {isCollapsed ? (
                    <ChevronRight aria-hidden="true" size={14} />
                  ) : (
                    <ChevronDown aria-hidden="true" size={14} />
                  )}
                </button>
                {isCollapsed ? (
                  <Folder
                    aria-hidden="true"
                    className="file-tree-icon"
                    size={15}
                  />
                ) : (
                  <FolderOpen
                    aria-hidden="true"
                    className="file-tree-icon"
                    size={15}
                  />
                )}
                <span>{row.name}</span>
                <span className="file-tree-count">{row.fileCount}</span>
              </span>
              <span className="file-tree-muted">目录</span>
              <span className="file-tree-muted">—</span>
              <span className="file-tree-muted">—</span>
            </>
          );
        }

        return (
          <>
            <span
              className="file-tree-cell file-tree-file"
              style={{ paddingLeft: `${row.depth * 18 + 18}px` }}
            >
              <FileCode2
                aria-hidden="true"
                className="file-tree-icon"
                size={15}
              />
              <span
                className="file-tree-file-name"
                title={row.file.relativePath}
              >
                {row.name}
              </span>
            </span>
            <span>{fileSourceStatusLabel(row.file)}</span>
            <span>—</span>
            <span>{fileResultStatusLabel(row.file.resultStatus)}</span>
          </>
        );
      }}
    />
  );
}

function buildFileTree(files: readonly FileRecordSummaryDto[]): DirectoryNode {
  const root: MutableDirectory = {
    kind: "directory",
    key: "",
    name: "",
    depth: -1,
    directories: [],
    directoryMap: new Map(),
    files: [],
    fileCount: 0,
  };

  for (const file of files) {
    const segments = file.relativePath.replaceAll("\\", "/").split("/");
    const fileName = segments.pop() || file.relativePath;
    let directory = root;
    for (const segment of segments) {
      const key = directory.key ? `${directory.key}/${segment}` : segment;
      let child = directory.directoryMap.get(segment);
      if (!child) {
        child = {
          kind: "directory",
          key,
          name: segment,
          depth: directory.depth + 1,
          directories: [],
          directoryMap: new Map(),
          files: [],
          fileCount: 0,
        };
        directory.directoryMap.set(segment, child);
        directory.directories.push(child);
      }
      directory = child;
    }
    directory.files.push({
      kind: "file",
      key: `file:${file.id}`,
      name: fileName,
      depth: directory.depth + 1,
      file,
    });
  }

  sortAndCount(root);
  return root;
}

function sortAndCount(directory: DirectoryNode): number {
  directory.directories.sort((left, right) =>
    left.name.localeCompare(right.name),
  );
  directory.files.sort((left, right) => left.name.localeCompare(right.name));
  directory.fileCount =
    directory.files.length +
    directory.directories.reduce(
      (total, child) => total + sortAndCount(child),
      0,
    );
  return directory.fileCount;
}

function flattenFileTree(
  root: DirectoryNode,
  collapsedDirectories: ReadonlySet<string>,
): TreeRow[] {
  const rows: TreeRow[] = [];
  const visit = (directory: DirectoryNode) => {
    if (directory.key) rows.push(directory);
    if (directory.key && collapsedDirectories.has(directory.key)) return;
    directory.directories.forEach(visit);
    rows.push(...directory.files);
  };
  visit(root);
  return rows;
}

function fileSourceStatusLabel(file: FileRecordSummaryDto): string {
  if (file.included)
    return file.sourceStatus === "modified" ? "已修改" : "待处理";
  if (file.exclusionReason) {
    return `已排除：${EXCLUSION_LABELS[file.exclusionReason] ?? file.exclusionReason}`;
  }
  switch (file.sourceStatus) {
    case "deleted":
      return "已删除";
    case "sensitive":
      return "敏感文件";
    case "unreadable":
      return "不可读取";
    case "unsupported_encoding":
      return "编码不支持";
    default:
      return "已排除";
  }
}

function fileResultStatusLabel(
  status: FileRecordSummaryDto["resultStatus"],
): string {
  switch (status) {
    case "current":
      return "当前结果";
    case "stale":
      return "结果过期";
    default:
      return "—";
  }
}
