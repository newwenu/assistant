import { useEffect, useState } from "react";
import { chatService } from "@/services/chat/chatService";
import { createLogger } from "@/services/system/logger";
import type { ToolDescriptorDto } from "@/types/protocol.contract";

const log = createLogger("toolCatalog");

/**
 * 工具清单是进程级事实（注册表在启动后不再变化），缓存必须活在模块作用域：
 * 挂在组件 `useRef` 上会随视图重挂载丢失，换布局就重拉一次。
 */
let cached: ToolDescriptorDto[] | null = null;
let inflight: Promise<ToolDescriptorDto[]> | null = null;

function loadToolCatalog(): Promise<ToolDescriptorDto[]> {
  if (cached) return Promise.resolve(cached);
  if (!inflight) {
    inflight = chatService
      .listAvailableTools()
      .then((tools) => {
        cached = tools;
        return tools;
      })
      .finally(() => {
        inflight = null;
      });
  }
  return inflight;
}

/** 只有 agent 链路需要工具清单；`enabled` 为 false 时不发请求。 */
export function useToolCatalog(enabled: boolean): {
  tools: ToolDescriptorDto[];
  loading: boolean;
} {
  const [tools, setTools] = useState<ToolDescriptorDto[]>(() => cached ?? []);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!enabled) return;
    if (cached) {
      setTools(cached);
      return;
    }
    let alive = true;
    setLoading(true);
    void loadToolCatalog()
      .then((list) => {
        if (alive) setTools(list);
      })
      .catch((e) => log.error("listAvailableTools failed", e))
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [enabled]);

  return { tools, loading };
}
