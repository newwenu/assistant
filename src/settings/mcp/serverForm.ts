import type {
  McpLaunchInputDto,
  McpServerDto,
  McpServerStatusDto,
} from "@/types/protocol.contract";

export const ENV_VALUE_MASK = "••••••";

export interface LaunchDraft {
  command: string;
  argsText: string;
  cwd: string;
  envText: string;
}

export interface EnvParseResult {
  env: Record<string, string>;
  invalidLines: string[];
}

export function parseArgs(text: string): string[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function formatArgs(args: readonly string[]): string {
  return args.join("\n");
}

export function parseEnv(text: string): EnvParseResult {
  const env: Record<string, string> = {};
  const invalidLines: string[] = [];
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const separator = line.indexOf("=");
    const key = separator > 0 ? line.slice(0, separator).trim() : "";
    if (!key) {
      invalidLines.push(line);
      continue;
    }
    env[key] = line.slice(separator + 1);
  }
  return { env, invalidLines };
}

export function maskEnvKeys(keys: readonly string[]): string {
  return keys.map((key) => `${key}=${ENV_VALUE_MASK}`).join("  ");
}

export function buildLaunch(draft: LaunchDraft): McpLaunchInputDto {
  const cwd = draft.cwd.trim();
  return {
    command: draft.command.trim(),
    args: parseArgs(draft.argsText),
    cwd: cwd.length > 0 ? cwd : null,
    env: parseEnv(draft.envText).env,
  };
}

export function isLaunchDirty(
  draft: LaunchDraft,
  server: McpServerDto,
): boolean {
  const next = buildLaunch(draft);
  return (
    next.command !== server.command ||
    next.args.length !== server.args.length ||
    next.args.some((arg, index) => arg !== server.args[index]) ||
    (next.cwd ?? null) !== server.cwd ||
    draft.envText.trim().length > 0
  );
}

export function requiresEnvRefill(
  draft: LaunchDraft,
  server: McpServerDto,
): boolean {
  return (
    server.envKeys.length > 0 &&
    draft.envText.trim().length === 0 &&
    isLaunchDirty(draft, server)
  );
}

export function statusLabelKey(status: McpServerStatusDto): string {
  return `settings.mcp.status.${status}`;
}

export function statusTagColor(status: McpServerStatusDto): string {
  switch (status) {
    case "ready":
      return "green";
    case "starting":
      return "blue";
    case "error":
      return "red";
    case "stopped":
      return "default";
  }
}
