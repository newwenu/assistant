import type { Provider } from "@/settings/provider/types";
import type { VisionSupport } from "./types";

/**
 * 从 provider / model 事实推导视觉能力三态，与后端同一口径：
 * `vision` 事实 → `inputModalities` → 旧字段 `supportVision`。
 * 模型未找到时保持 `unsupported`：未选模型时不冒出图片入口。
 */
export function resolveVisionSupport(
  providers: Provider[],
  providerId: string | null,
  modelId: string,
): VisionSupport {
  const provider = providers.find((p) => p.id === providerId);
  const model = provider?.models.find((m) => m.id === modelId);
  if (!model) return "unsupported";
  if (model.vision) return model.vision.support;
  if (model.inputModalities.length > 0) {
    return model.inputModalities.includes("image")
      ? ("supported" as const)
      : ("unsupported" as const);
  }
  return model.supportVision
    ? ("supported" as const)
    : ("unsupported" as const);
}
