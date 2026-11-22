/**
 * 示例卡片类别表（I68 §7.1 / A1）：画布与放置栏共用的唯一来源。
 * 形按词表定义做全，内容全是示例常量（原语量），不接运行、不接后端；
 * 类别名与说明走 i18n 键，示例值不进 i18n（§8.3）。
 */
import type { CardKindId, CardKindSpec } from "./types";

export const CARD_KINDS = {
  "execution-turn": {
    id: "execution-turn",
    category: "execution",
    formKey: "canvas.card.form.turn",
    nameKey: "canvas.card.name.turn",
    summaryKey: "canvas.card.summary.turn",
    reference: "turn",
    params: [
      { labelKey: "canvas.card.param.model", value: "default" },
      { labelKey: "canvas.card.param.budget", value: "64k" },
      { labelKey: "canvas.card.param.toolset", value: "3" },
    ],
    ports: [
      { side: "left", typeLabel: "turn.in" },
      { side: "right", typeLabel: "turn.out" },
    ],
    body: "stages",
    tone: "active",
    exit: "stopped",
  },
  "execution-tool": {
    id: "execution-tool",
    category: "execution",
    formKey: "canvas.card.form.tool",
    nameKey: "canvas.card.name.tool",
    summaryKey: "canvas.card.summary.tool",
    reference: "shell",
    params: [
      { labelKey: "canvas.card.param.command", value: "rg -n TODO" },
      { labelKey: "canvas.card.param.timeout", value: "30s" },
      { labelKey: "canvas.card.param.sandbox", value: "workspace" },
    ],
    ports: [
      { side: "left", typeLabel: "tool.in" },
      { side: "right", typeLabel: "tool.out" },
    ],
    body: "output",
    tone: "success",
  },
  "execution-conversation": {
    id: "execution-conversation",
    category: "execution",
    formKey: "canvas.card.form.conversation",
    nameKey: "canvas.card.name.conversation",
    summaryKey: "canvas.card.summary.conversation",
    reference: "conversation",
    params: [
      { labelKey: "canvas.card.param.session_key", value: "conv-1" },
      { labelKey: "canvas.card.param.retain", value: "50" },
      { labelKey: "canvas.card.param.messages", value: "12" },
    ],
    ports: [
      { side: "left", typeLabel: "message" },
      { side: "right", typeLabel: "message" },
    ],
    body: "messages",
    tone: "default",
  },
  "execution-hitl": {
    id: "execution-hitl",
    category: "execution",
    formKey: "canvas.card.form.hitl",
    nameKey: "canvas.card.name.hitl",
    summaryKey: "canvas.card.summary.hitl",
    reference: "ask_user",
    params: [
      { labelKey: "canvas.card.param.question", value: "ask_user" },
      { labelKey: "canvas.card.param.options", value: "2" },
      { labelKey: "canvas.card.param.wait", value: "5m" },
    ],
    ports: [
      { side: "left", typeLabel: "question" },
      { side: "right", typeLabel: "answer" },
    ],
    body: "question",
    tone: "warning",
  },
  "container-conversation": {
    id: "container-conversation",
    category: "container",
    formKey: null,
    nameKey: "canvas.card.name.container",
    summaryKey: "canvas.card.summary.container",
    reference: "subgraph",
    params: [
      { labelKey: "canvas.card.param.subgraph_nodes", value: "6" },
      { labelKey: "canvas.card.param.iteration_limit", value: "8" },
      { labelKey: "canvas.card.param.level", value: "conversation" },
    ],
    ports: [
      { side: "left", typeLabel: "subgraph.in" },
      { side: "right", typeLabel: "subgraph.out" },
    ],
    body: "subgraph",
    tone: "muted",
  },
} as const satisfies Record<CardKindId, CardKindSpec>;

/** 声明顺序：执行卡片四型 → 容器卡片（放置栏条目顺序与之一致，A24）。 */
export const CARD_KIND_IDS = [
  "execution-turn",
  "execution-tool",
  "execution-conversation",
  "execution-hitl",
  "container-conversation",
] as const satisfies readonly CardKindId[];
