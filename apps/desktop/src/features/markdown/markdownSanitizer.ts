export function sanitizeMarkdown(markdown: string): string {
  return markdown
    .replace(/<[^>]*>/g, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "[图片已隐藏]")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\b(?:javascript|file|data):[^\s)]+/gi, "[链接已隐藏]");
}
