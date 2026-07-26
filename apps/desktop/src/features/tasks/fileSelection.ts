import type { FileRecordSummaryDto } from "@batch-code-analyzer/ipc-types";

export function canIncludeFile(file: FileRecordSummaryDto): boolean {
  if (file.sourceStatus !== "normal" && file.sourceStatus !== "modified") {
    return false;
  }
  return ![
    "binary",
    "builtin_extension",
    "file_too_large",
    "sensitive",
    "sensitive_content",
    "sensitive_filename",
    "symlink",
    "unreadable",
    "unsupported_encoding",
  ].includes(file.exclusionReason ?? "");
}
