import { X } from "lucide-react";

import { sanitizeMarkdown } from "./markdownSanitizer";

interface MarkdownPreviewProps {
  content: string;
  open: boolean;
  title?: string;
  onClose: () => void;
}

/**
 * A deliberately conservative preview for the shell. It renders text only,
 * stripping raw HTML, image URLs, and link destinations before display.
 */
export function MarkdownPreview({
  content,
  open,
  title = "Markdown 预览",
  onClose,
}: MarkdownPreviewProps) {
  if (!open) return null;

  return (
    <div className="preview-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        aria-label={title}
        aria-modal="true"
        className="preview-dialog"
        role="dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="preview-dialog-header">
          <div>
            <p className="eyebrow">RESULT PREVIEW</p>
            <h2>{title}</h2>
          </div>
          <button
            aria-label="关闭预览"
            className="icon-button"
            onClick={onClose}
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </div>
        <pre className="markdown-content">{sanitizeMarkdown(content)}</pre>
      </section>
    </div>
  );
}
