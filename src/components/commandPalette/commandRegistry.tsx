import React from "react";
import {
  ThunderboltOutlined,
  AppstoreOutlined,
  BulbOutlined,
  GlobalOutlined,
} from "@ant-design/icons";

export type CommandCategory = "tools" | "navigation" | "actions" | "external";

export interface CommandCategoryMeta {
  key: CommandCategory;
  label: string;
  icon: React.ReactNode;
  order: number;
}

export interface CommandDefinition {
  id: string;
  name: string;
  description?: string;
  keywords: string[];
  category: CommandCategory;
  icon?: React.ReactNode;
  action: () => void | Promise<void>;
  closeOnSelect?: boolean;
}

export const COMMAND_CATEGORIES: readonly CommandCategoryMeta[] = [
  {
    key: "tools",
    label: "command.category.tools",
    icon: <ThunderboltOutlined />,
    order: 0,
  },
  {
    key: "navigation",
    label: "command.category.navigation",
    icon: <AppstoreOutlined />,
    order: 1,
  },
  {
    key: "actions",
    label: "command.category.actions",
    icon: <BulbOutlined />,
    order: 2,
  },
  {
    key: "external",
    label: "command.category.external",
    icon: <GlobalOutlined />,
    order: 3,
  },
];

const COMMAND_CATEGORY_MAP = new Map<CommandCategory, CommandCategoryMeta>(
  COMMAND_CATEGORIES.map((c) => [c.key, c]),
);

export function getCategoryMeta(key: CommandCategory): CommandCategoryMeta {
  return COMMAND_CATEGORY_MAP.get(key)!;
}

const commandStore: CommandDefinition[] = [];

export function defineCommand(cmd: CommandDefinition): CommandDefinition {
  commandStore.push(cmd);
  return cmd;
}

export function getAllCommands(): readonly CommandDefinition[] {
  return commandStore;
}

export function getCommandsByCategory(
  category: CommandCategory,
): readonly CommandDefinition[] {
  return commandStore.filter((c) => c.category === category);
}

export function getCommandById(id: string): CommandDefinition | undefined {
  return commandStore.find((c) => c.id === id);
}

export function clearCommands(): void {
  commandStore.length = 0;
}
