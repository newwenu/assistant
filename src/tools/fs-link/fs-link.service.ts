export type LinkKind = "symlink" | "hardlink" | "junction";

function buildLinkKindOptions(): { value: LinkKind; label: string }[] {
  const opts: { value: LinkKind; label: string }[] = [
    { value: "symlink", label: "符号链接 (Symlink)" },
    { value: "hardlink", label: "硬链接 (Hardlink)" },
  ];
  if (
    typeof navigator !== "undefined" &&
    navigator.platform.startsWith("Win")
  ) {
    opts.push({ value: "junction", label: "目录联接 (Junction)" });
  }
  return opts;
}

export const LINK_KIND_OPTIONS = buildLinkKindOptions();

export const KIND_DESCRIPTIONS: Record<LinkKind, string> = {
  symlink:
    "符号链接：创建一个指向目标的引用路径，类似快捷方式。删除源链接不影响目标。",
  hardlink:
    "硬链接：为同一文件数据创建额外目录项。源和目标共享相同磁盘数据块，删除任一方不影响另一方数据。仅支持文件，不支持目录。",
  junction:
    "目录联接：将源目录映射到目标目录，使源路径下的内容等同于目标路径。仅支持目录，不需要开发者模式特权。",
};

export interface CheckResult {
  source: string;
  isLink: boolean;
  kind: string | null;
  target: string | null;
}

export interface CreateResult {
  source: string;
  target: string;
  kind: string;
  created: boolean;
}
