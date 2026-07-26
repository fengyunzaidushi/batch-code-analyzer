import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ChevronDown,
  ChevronRight,
  FileCode2,
  Folder,
  FolderOpen,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { FileRecordSummaryDto } from "@batch-code-analyzer/ipc-types";

import { canIncludeFile } from "./fileSelection";
import { VirtualTaskTable } from "./VirtualTaskTable";

interface FileTreeTableProps {
  files: readonly FileRecordSummaryDto[];
  emptyLabel?: string;
  onSetIncluded?: (
    file: FileRecordSummaryDto,
    included: boolean,
  ) => Promise<void>;
  onAuthorizeSensitive?: (fileId: string) => Promise<void>;
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
  builtin_filename: "低价值文件",
  file_too_large: "文件过大",
  gitignore_or_user_pattern: "被忽略规则排除",
  not_included_extension: "不在纳入扩展名内",
  sensitive: "敏感文件",
  sensitive_content: "检测到敏感内容",
  sensitive_filename: "敏感文件名",
  symlink: "符号链接",
  unreadable: "无法读取",
  unsupported_encoding: "编码不支持",
  user_excluded: "用户手动排除",
};

const TOKEN_ESTIMATION_BYTES_PER_TOKEN = 2;
const LONG_FILE_TOKEN_WARNING_THRESHOLD = 10_000;

type FileSortKey = "status" | "tokens";
type FileSortDirection = "asc" | "desc";
type FileSort = { key: FileSortKey; direction: FileSortDirection } | null;

export function FileTreeTable({
  files,
  emptyLabel = "暂无文件",
  onSetIncluded,
  onAuthorizeSensitive,
}: FileTreeTableProps) {
  const [collapsedDirectories, setCollapsedDirectories] = useState<Set<string>>(
    () => new Set(),
  );
  const [updatingFileId, setUpdatingFileId] = useState<string | null>(null);
  const [fileSort, setFileSort] = useState<FileSort>(null);
  const root = useMemo(() => buildFileTree(files), [files]);
  const treeRows = useMemo(
    () => flattenFileTree(root, collapsedDirectories),
    [collapsedDirectories, root],
  );
  const rows = useMemo(
    () => (fileSort ? sortFileRows(files, fileSort) : treeRows),
    [fileSort, files, treeRows],
  );

  const toggleDirectory = (key: string) => {
    setCollapsedDirectories((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const toggleFileSort = (key: FileSortKey) => {
    setFileSort((current) => ({
      direction:
        current?.key === key && current.direction === "asc" ? "desc" : "asc",
      key,
    }));
  };

  const toggleFile = async (file: FileRecordSummaryDto, included: boolean) => {
    if (!onSetIncluded) return;
    setUpdatingFileId(file.id);
    try {
      await onSetIncluded(file, included);
    } catch {
      // The application layer owns the user-facing error; this only clears the busy state.
    } finally {
      setUpdatingFileId(null);
    }
  };

  const authorizeSensitive = async (file: FileRecordSummaryDto) => {
    if (!onAuthorizeSensitive || file.included) return;
    const confirmed = window.confirm(
      `文件“${file.relativePath}”被检测为敏感文件。授权后其当前内容可能发送给模型，确定继续吗？`,
    );
    if (!confirmed) return;
    setUpdatingFileId(file.id);
    try {
      await onAuthorizeSensitive(file.id);
    } catch {
      // The application layer owns the user-facing error; this only clears the busy state.
    } finally {
      setUpdatingFileId(null);
    }
  };

  return (
    <VirtualTaskTable
      ariaLabel="扫描文件列表"
      className="file-tree-table"
      emptyLabel={emptyLabel}
      getRowKey={(row) => row.key}
      header={
        <>
          <span role="columnheader">文件</span>
          <SortableFileHeader
            activeSort={fileSort}
            label="状态"
            onSort={() => toggleFileSort("status")}
            sortKey="status"
          />
          <SortableFileHeader
            activeSort={fileSort}
            label="大小 / 预估 Token"
            onSort={() => toggleFileSort("tokens")}
            sortKey="tokens"
          />
          <span role="columnheader">模型</span>
          <span role="columnheader">结果</span>
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
              <span className="file-tree-muted">—</span>
            </>
          );
        }

        const estimatedTokens = estimateFileTokens(row.file.sizeBytes);
        const isLongFile = estimatedTokens > LONG_FILE_TOKEN_WARNING_THRESHOLD;

        return (
          <>
            <span
              className="file-tree-cell file-tree-file"
              style={{ paddingLeft: `${row.depth * 18 + 18}px` }}
            >
              <input
                aria-label={`${row.file.included ? "排除" : "纳入"}文件 ${row.file.relativePath}`}
                checked={row.file.included}
                className="file-tree-checkbox"
                disabled={
                  !onSetIncluded ||
                  updatingFileId === row.file.id ||
                  (!row.file.included && !canIncludeFile(row.file))
                }
                onChange={(event) => {
                  void toggleFile(row.file, event.currentTarget.checked);
                }}
                type="checkbox"
              />
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
            <span className="file-status-cell">
              {fileSourceStatusLabel(row.file)}
              {row.file.sourceStatus === "sensitive" &&
              !row.file.included &&
              onAuthorizeSensitive ? (
                <button
                  aria-label={`授权并纳入文件 ${row.file.relativePath}`}
                  className="file-sensitive-authorize"
                  disabled={updatingFileId === row.file.id}
                  onClick={() => void authorizeSensitive(row.file)}
                  title="确认后允许将当前敏感文件纳入分析"
                  type="button"
                >
                  <ShieldCheck aria-hidden="true" size={13} />
                  授权纳入
                </button>
              ) : null}
            </span>
            <span
              className={`file-size-cell${isLongFile ? " is-warning" : ""}`}
              title={
                isLongFile
                  ? `预估 ${formatTokenCount(estimatedTokens)} tokens，代码文件过长，建议排除、拆分或更换上下文更大的模型`
                  : `预估 ${formatTokenCount(estimatedTokens)} tokens`
              }
            >
              <span>{formatFileSize(row.file.sizeBytes)}</span>
              <span className="file-token-estimate">
                {isLongFile ? (
                  <TriangleAlert aria-hidden="true" size={12} />
                ) : null}
                约 {formatTokenCount(estimatedTokens)} tokens
                {isLongFile ? " · 代码文件过长" : null}
              </span>
            </span>
            <span>—</span>
            <span>{fileResultStatusLabel(row.file.resultStatus)}</span>
          </>
        );
      }}
    />
  );
}

function SortableFileHeader({
  activeSort,
  label,
  onSort,
  sortKey,
}: {
  activeSort: FileSort;
  label: string;
  onSort: () => void;
  sortKey: FileSortKey;
}) {
  const direction = activeSort?.key === sortKey ? activeSort.direction : null;
  const nextDirection = direction === "asc" ? "desc" : "asc";
  const SortIcon =
    direction === "asc"
      ? ArrowUp
      : direction === "desc"
        ? ArrowDown
        : ArrowUpDown;
  const nextDirectionLabel = nextDirection === "asc" ? "升序" : "降序";

  return (
    <span
      aria-sort={
        direction === "asc"
          ? "ascending"
          : direction === "desc"
            ? "descending"
            : undefined
      }
      className="task-table-column-header"
      role="columnheader"
    >
      <button
        aria-label={`按文件列表${label}${nextDirectionLabel}排序`}
        className={`task-table-sort-button${direction ? " is-active" : ""}`}
        onClick={onSort}
        title={`按文件列表${label}${nextDirectionLabel}排序`}
        type="button"
      >
        <span>{label}</span>
        <SortIcon aria-hidden="true" size={13} />
      </button>
    </span>
  );
}

function estimateFileTokens(sizeBytes: number): number {
  return Math.ceil(Math.max(0, sizeBytes) / TOKEN_ESTIMATION_BYTES_PER_TOKEN);
}

function formatFileSize(sizeBytes: number): string {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${Math.round(sizeBytes / 1024)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTokenCount(tokenCount: number): string {
  return new Intl.NumberFormat("zh-CN").format(tokenCount);
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

function sortFileRows(
  files: readonly FileRecordSummaryDto[],
  sort: Exclude<FileSort, null>,
): FileNode[] {
  return files
    .map((file, originalIndex) => ({ file, originalIndex }))
    .sort((left, right) => {
      const comparison =
        sort.key === "tokens"
          ? estimateFileTokens(left.file.sizeBytes) -
            estimateFileTokens(right.file.sizeBytes)
          : fileStatusSortRank(left.file) - fileStatusSortRank(right.file);
      if (comparison !== 0) {
        return sort.direction === "asc" ? comparison : -comparison;
      }
      const pathComparison = left.file.relativePath.localeCompare(
        right.file.relativePath,
      );
      return pathComparison === 0
        ? left.originalIndex - right.originalIndex
        : pathComparison;
    })
    .map(({ file }) => ({
      kind: "file",
      key: `file:${file.id}`,
      name: file.relativePath.replaceAll("\\", "/"),
      depth: 0,
      file,
    }));
}

function fileStatusSortRank(file: FileRecordSummaryDto): number {
  if (
    file.sourceStatus === "sensitive" ||
    file.exclusionReason?.startsWith("sensitive")
  ) {
    return 0;
  }
  if (file.sourceStatus === "unreadable") return 1;
  if (file.sourceStatus === "unsupported_encoding") return 2;
  if (file.exclusionReason === "binary") return 3;
  if (file.exclusionReason === "file_too_large") return 4;
  if (file.sourceStatus === "deleted") return 5;
  if (file.sourceStatus === "modified") return 6;
  if (file.included) return 7;
  return 8;
}

function fileSourceStatusLabel(file: FileRecordSummaryDto): string {
  if (file.included && file.sourceStatus === "sensitive")
    return "已授权，待处理";
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
