import { diffLines, diffChars } from "diff";

export interface DiffLine {
  type: "add" | "remove" | "same";
  content: string;
  lineNumOld?: number;
  lineNumNew?: number;
  charParts?: { text: string; added?: boolean; removed?: boolean }[];
}

export const LINE_COLORS: Record<string, string> = {
  add: "var(--diff-add-bg)",
  remove: "var(--diff-remove-bg)",
  same: "transparent",
};

export const PREFIX: Record<string, string> = {
  add: "+ ",
  remove: "- ",
  same: "  ",
};

export function computeDiff(oldText: string, newText: string): DiffLine[] {
  const changes = diffLines(oldText, newText);
  const result: DiffLine[] = [];
  let lineNumOld = 1;
  let lineNumNew = 1;

  for (const change of changes) {
    const lines = change.value.replace(/\n$/, "").split("\n");
    for (const line of lines) {
      if (change.added) {
        let charParts: DiffLine["charParts"];
        if (result.length > 0) {
          const prev = result[result.length - 1];
          if (prev.type === "remove" && prev.content === line) {
            const charDiff = diffChars(prev.content, line);
            prev.charParts = charDiff
              .filter((c) => !c.added)
              .map((c) => ({
                text: c.value,
                removed: !c.added && !c.removed ? undefined : c.removed,
              }));
            prev.type = "same";
            prev.lineNumNew = lineNumNew;
            lineNumNew++;
            lineNumOld++;
            continue;
          }
        }
        result.push({
          type: "add",
          content: line,
          lineNumNew: lineNumNew++,
          charParts,
        });
      } else if (change.removed) {
        result.push({
          type: "remove",
          content: line,
          lineNumOld: lineNumOld++,
        });
      } else {
        result.push({
          type: "same",
          content: line,
          lineNumOld: lineNumOld++,
          lineNumNew: lineNumNew++,
        });
      }
    }
  }

  const final: DiffLine[] = [];
  let i = 0;
  while (i < result.length) {
    if (result[i].type === "remove") {
      const removeGroup: DiffLine[] = [];
      while (i < result.length && result[i].type === "remove") {
        removeGroup.push(result[i++]);
      }
      const addGroup: DiffLine[] = [];
      while (i < result.length && result[i].type === "add") {
        addGroup.push(result[i++]);
      }
      const pairLen = Math.min(removeGroup.length, addGroup.length);
      for (let p = 0; p < pairLen; p++) {
        const charDiff = diffChars(removeGroup[p].content, addGroup[p].content);
        final.push({
          type: "remove",
          content: removeGroup[p].content,
          lineNumOld: removeGroup[p].lineNumOld,
          charParts: charDiff
            .filter((c) => !c.added)
            .map((c) => ({
              text: c.value,
              removed: c.removed || undefined,
            })),
        });
        final.push({
          type: "add",
          content: addGroup[p].content,
          lineNumNew: addGroup[p].lineNumNew,
          charParts: charDiff
            .filter((c) => !c.removed)
            .map((c) => ({
              text: c.value,
              added: c.added || undefined,
            })),
        });
      }
      for (let p = pairLen; p < removeGroup.length; p++) {
        final.push(removeGroup[p]);
      }
      for (let p = pairLen; p < addGroup.length; p++) {
        final.push(addGroup[p]);
      }
    } else {
      final.push(result[i++]);
    }
  }

  return final;
}
