import React, {
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
} from "react";
import { Input, Typography } from "antd";
import type { InputRef } from "antd";
import { SearchOutlined } from "@ant-design/icons";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import Fuse from "fuse.js";
import { translateKey } from "@/i18n";
import {
  getAllCommands,
  COMMAND_CATEGORIES,
  type CommandDefinition,
} from "./commandRegistry";
import { aggregateCommands } from "./commandAggregation";
import { useCommandPaletteShortcut } from "./commandPaletteBindings";
import { COMMAND_PALETTE_MAX_PER_CATEGORY } from "@/config/constants";
import { useConfGroup } from "@/settings/core/useConfGroup";
import { IS_MAC } from "@/settings/core/types/shortcutTypes";
import styles from "./CommandPalette.module.css";

export const CommandPalette: React.FC = () => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const inputRef = useRef<InputRef>(null);
  const optionRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const navigate = useNavigate();
  const { t, i18n: currentI18n } = useTranslation();
  const appConf = useConfGroup("app");
  const binding = appConf?.keybindings?.commandPalette;

  const allCommands = useMemo(() => {
    void currentI18n.language;
    aggregateCommands(navigate);
    return [...getAllCommands()];
  }, [navigate, currentI18n.language]);

  const fuse = useMemo(
    () =>
      new Fuse(allCommands, {
        keys: [
          { name: "name", weight: 10 },
          { name: "keywords", weight: 1 },
          { name: "description", weight: 0.5 },
        ],
        threshold: 0.3,
      }),
    [allCommands],
  );

  const fuseResults = useMemo(() => {
    if (!query.trim()) return undefined;
    return fuse.search(query);
  }, [query, fuse]);

  const groupedResults = useMemo(() => {
    const items = fuseResults ? fuseResults.map((r) => r.item) : allCommands;
    const groups: Record<string, CommandDefinition[]> = {};
    for (const cmd of items) {
      if (!groups[cmd.category]) groups[cmd.category] = [];
      if (groups[cmd.category].length < COMMAND_PALETTE_MAX_PER_CATEGORY) {
        groups[cmd.category].push(cmd);
      }
    }
    return groups;
  }, [fuseResults, allCommands]);

  const sortedGroups = useMemo(
    () =>
      Object.entries(groupedResults).sort(([a], [b]) => {
        const ao = COMMAND_CATEGORIES.find((c) => c.key === a)?.order ?? 99;
        const bo = COMMAND_CATEGORIES.find((c) => c.key === b)?.order ?? 99;
        return ao - bo;
      }),
    [groupedResults],
  );

  const flatResults = useMemo(
    () => sortedGroups.flatMap(([, cmds]) => cmds),
    [sortedGroups],
  );

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
    setSelectedIndex(-1);
  }, []);

  const toggle = useCallback(() => {
    setOpen((v) => {
      if (!v) {
        setQuery("");
        setSelectedIndex(-1);
      }
      return !v;
    });
  }, []);

  const activate = useCallback(
    (cmd: CommandDefinition, keepOpen = false) => {
      void cmd.action();
      if (cmd.closeOnSelect !== false && !keepOpen) close();
    },
    [close],
  );

  useCommandPaletteShortcut(binding, toggle);

  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    if (!open || selectedIndex < 0 || flatResults.length === 0) return;
    const cmd = flatResults[selectedIndex];
    if (!cmd) return;
    const el = optionRefs.current.get(cmd.id);
    el?.scrollIntoView({ block: "nearest" });
  }, [open, selectedIndex, flatResults]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    const ctrlOrCmd = IS_MAC ? e.metaKey : e.ctrlKey;

    if (e.key === "ArrowDown" || (ctrlOrCmd && e.key.toLowerCase() === "n")) {
      e.preventDefault();
      setSelectedIndex((i) =>
        i < 0 ? 0 : Math.min(i + 1, flatResults.length - 1),
      );
    } else if (
      e.key === "ArrowUp" ||
      (ctrlOrCmd && e.key.toLowerCase() === "p")
    ) {
      e.preventDefault();
      setSelectedIndex((i) => (i <= 0 ? -1 : i - 1));
    } else if (e.key === "Tab") {
      e.preventDefault();
      if (e.shiftKey) {
        setSelectedIndex((i) => {
          if (i <= 0) return -1;
          const cur = flatResults[i];
          let prevCat: string | null = null;
          for (let j = i - 1; j >= 0; j--) {
            if (flatResults[j].category !== cur.category) {
              prevCat = flatResults[j].category;
              break;
            }
          }
          if (!prevCat) return -1;
          for (let j = i - 1; j >= 0; j--) {
            if (flatResults[j].category === prevCat) return j;
          }
          return -1;
        });
      } else {
        setSelectedIndex((i) => {
          const start = i < 0 ? 0 : i;
          const curCat = flatResults[start]?.category;
          for (let j = start + 1; j < flatResults.length; j++) {
            if (flatResults[j].category !== curCat) return j;
          }
          return i < 0 ? 0 : i;
        });
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (selectedIndex >= 0) {
        activate(flatResults[selectedIndex], e.ctrlKey || e.metaKey);
      } else if (flatResults.length > 0) {
        setSelectedIndex(0);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (query) {
        setQuery("");
        setSelectedIndex(-1);
      } else {
        close();
      }
    } else if (
      /^[1-9]$/.test(e.key) &&
      selectedIndex >= 0 &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey &&
      !e.shiftKey
    ) {
      const idx = parseInt(e.key, 10) - 1;
      if (idx < flatResults.length) {
        e.preventDefault();
        activate(flatResults[idx]);
      }
    }
  };

  if (!open) return null;

  return (
    <div className={styles.overlay} onClick={close}>
      <div className={styles.panel} onClick={(e) => e.stopPropagation()}>
        <div className={styles.inputWrapper}>
          <Input
            ref={inputRef}
            prefix={<SearchOutlined />}
            placeholder={t("command.search_placeholder")}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(-1);
            }}
            onKeyDown={handleKeyDown}
            size="large"
            variant="borderless"
          />
        </div>
        <div className={styles.resultList}>
          {flatResults.length === 0 && (
            <div className={styles.emptyState}>
              <SearchOutlined style={{ fontSize: 24, opacity: 0.3 }} />
              <Typography.Text type="secondary">
                {t("command.no_match")}
              </Typography.Text>
            </div>
          )}
          {sortedGroups.map(([catKey, cmds]) => {
            const meta = COMMAND_CATEGORIES.find((c) => c.key === catKey);
            return (
              <div key={catKey}>
                <div className={styles.categoryTitle}>
                  {meta?.icon} {translateKey(meta?.label ?? catKey)}
                </div>
                {cmds.map((cmd) => {
                  const flatIdx = flatResults.indexOf(cmd);
                  const isActive = flatIdx === selectedIndex;
                  return (
                    <div
                      key={cmd.id}
                      ref={(el) => {
                        if (el) optionRefs.current.set(cmd.id, el);
                        else optionRefs.current.delete(cmd.id);
                      }}
                      className={`${styles.option} ${isActive ? styles.optionActive : ""}`}
                      onClick={() => activate(cmd)}
                      onMouseEnter={() => setSelectedIndex(flatIdx)}
                    >
                      <span className={styles.optionName}>
                        {cmd.icon && (
                          <span className={styles.optionIcon}>{cmd.icon}</span>
                        )}
                        {cmd.name}
                      </span>
                      {cmd.description && (
                        <Typography.Text
                          type="secondary"
                          className={styles.optionDesc}
                        >
                          {cmd.description}
                        </Typography.Text>
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
