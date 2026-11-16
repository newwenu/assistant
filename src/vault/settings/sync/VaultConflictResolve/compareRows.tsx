import type { ReactNode } from "react";
import { Typography } from "antd";
import type { TFunction } from "i18next";

import { vaultCategoryLabel } from "../../../core/display/vaultCategory";
import type { EntryPreview } from "../../../core/vault.types";

import { emptyValue, renderSecret, renderTags } from "./secretCells";
import { formatTime, type CompareRow, type RevealController } from "./types";

const { Text } = Typography;

/**
 * 把两侧条目按字段摊平成对比行：`diff` 决定哪些行算「有变更」，
 * 缺失的一侧统一落到空值占位。
 */
export function buildRows(
  local: EntryPreview,
  remote: EntryPreview,
  diff: Set<string>,
  t: TFunction,
  reveal: RevealController,
): CompareRow[] {
  const rows: CompareRow[] = [];
  const empty = emptyValue(t);

  const push = (
    key: string,
    label: string,
    localPresent: boolean,
    remotePresent: boolean,
    localNode: ReactNode,
    remoteNode: ReactNode,
  ) => {
    rows.push({
      key,
      label,
      local: localPresent ? localNode : empty,
      remote: remotePresent ? remoteNode : empty,
      localPresent,
      remotePresent,
      changed: diff.has(key),
    });
  };

  push(
    "title",
    t("vault.entry.title"),
    true,
    true,
    <Text>{local.title}</Text>,
    <Text>{remote.title}</Text>,
  );
  push(
    "category",
    t("vault.entry.category"),
    true,
    true,
    <Text>{vaultCategoryLabel(t, local.category)}</Text>,
    <Text>{vaultCategoryLabel(t, remote.category)}</Text>,
  );
  push(
    "username",
    t("vault.entry.username"),
    local.username !== null,
    remote.username !== null,
    <Text>{local.username}</Text>,
    <Text>{remote.username}</Text>,
  );
  push(
    "url",
    t("vault.entry.url"),
    local.url !== null,
    remote.url !== null,
    <Text>{local.url}</Text>,
    <Text>{remote.url}</Text>,
  );
  push(
    "password",
    t("vault.entry.password"),
    local.password !== null,
    remote.password !== null,
    renderSecret(reveal, "local", "password", local.password ?? "", t),
    renderSecret(reveal, "remote", "password", remote.password ?? "", t),
  );
  push(
    "totpSecret",
    t("vault.entry.totp_secret"),
    local.totpSecret !== null,
    remote.totpSecret !== null,
    renderSecret(reveal, "local", "totpSecret", local.totpSecret ?? "", t),
    renderSecret(reveal, "remote", "totpSecret", remote.totpSecret ?? "", t),
  );
  push(
    "notes",
    t("vault.entry.notes"),
    local.notes !== null,
    remote.notes !== null,
    <Text style={{ whiteSpace: "pre-wrap" }}>{local.notes}</Text>,
    <Text style={{ whiteSpace: "pre-wrap" }}>{remote.notes}</Text>,
  );

  const customNames = Array.from(
    new Set([
      ...local.customFields.map((field) => field.name),
      ...remote.customFields.map((field) => field.name),
    ]),
  );
  for (const name of customNames) {
    const localField = local.customFields.find((field) => field.name === name);
    const remoteField = remote.customFields.find(
      (field) => field.name === name,
    );
    push(
      `custom:${name}`,
      name,
      localField !== undefined,
      remoteField !== undefined,
      localField?.isSecret ? (
        renderSecret(reveal, "local", name, localField.value, t)
      ) : (
        <Text>{localField?.value}</Text>
      ),
      remoteField?.isSecret ? (
        renderSecret(reveal, "remote", name, remoteField.value, t)
      ) : (
        <Text>{remoteField?.value}</Text>
      ),
    );
  }

  push(
    "tags",
    t("vault.entry.tags"),
    true,
    true,
    renderTags(local.tags, t),
    renderTags(remote.tags, t),
  );
  push(
    "revision",
    t("vault.history.revision"),
    true,
    true,
    <Text>{local.revision}</Text>,
    <Text>{remote.revision}</Text>,
  );
  push(
    "updatedAt",
    t("vault.history.time"),
    true,
    true,
    <Text>{formatTime(local.updatedAt)}</Text>,
    <Text>{formatTime(remote.updatedAt)}</Text>,
  );
  push(
    "commitAuthor",
    t("vault.conflict.commit_author"),
    true,
    true,
    <Text>{local.commitAuthor ?? t("vault.conflict.value_empty")}</Text>,
    <Text>{remote.commitAuthor ?? t("vault.conflict.value_empty")}</Text>,
  );
  push(
    "commitTime",
    t("vault.conflict.commit_time"),
    true,
    true,
    <Text>
      {local.commitTime !== null
        ? formatTime(local.commitTime)
        : t("vault.conflict.value_empty")}
    </Text>,
    <Text>
      {remote.commitTime !== null
        ? formatTime(remote.commitTime)
        : t("vault.conflict.value_empty")}
    </Text>,
  );

  return rows;
}
