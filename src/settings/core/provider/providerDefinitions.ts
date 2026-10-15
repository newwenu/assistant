import { defineProviderFormat } from "./providerFormatRegistry";
import { defineProviderPreset } from "./providerPresetRegistry";

defineProviderFormat({
  id: "openai",
  label: "settings.provider.format.openai",
  description: "settings.provider.format.openai.description",
  needsApiKey: true,
  keyPlaceholder: "sk-...",
  color: "var(--color-primary)",
});

defineProviderFormat({
  id: "anthropic",
  label: "settings.provider.format.anthropic",
  description: "settings.provider.format.anthropic.description",
  needsApiKey: true,
  keyPlaceholder: "sk-ant-...",
  color: "var(--color-format-anthropic)",
});

defineProviderFormat({
  id: "local",
  label: "settings.provider.format.local",
  description: "settings.provider.format.local.description",
  needsApiKey: false,
  color: "var(--color-success)",
});

defineProviderPreset({
  slug: "openai",
  label: "settings.provider.preset.openai",
  format: "openai",
  defaultBaseUrl: "https://api.openai.com/v1",
});

defineProviderPreset({
  slug: "deepseek",
  label: "settings.provider.preset.deepseek",
  format: "openai",
  defaultBaseUrl: "https://api.deepseek.com/v1",
});

defineProviderPreset({
  slug: "groq",
  label: "settings.provider.preset.groq",
  format: "openai",
  defaultBaseUrl: "https://api.groq.com/openai/v1",
});

defineProviderPreset({
  slug: "openrouter",
  label: "settings.provider.preset.openrouter",
  format: "openai",
  defaultBaseUrl: "https://openrouter.ai/api/v1",
});

defineProviderPreset({
  slug: "mistral",
  label: "settings.provider.preset.mistral",
  format: "openai",
  defaultBaseUrl: "https://api.mistral.ai/v1",
});

defineProviderPreset({
  slug: "zhipu",
  label: "settings.provider.preset.zhipu",
  format: "openai",
  defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
});

defineProviderPreset({
  slug: "qwen",
  label: "settings.provider.preset.qwen",
  format: "openai",
  defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
});

defineProviderPreset({
  slug: "anthropic",
  label: "settings.provider.preset.anthropic",
  format: "anthropic",
  defaultBaseUrl: "https://api.anthropic.com/v1",
});

defineProviderPreset({
  slug: "ollama",
  label: "settings.provider.preset.ollama",
  format: "local",
  defaultBaseUrl: "http://localhost:11434/v1",
});

defineProviderPreset({
  slug: "lmstudio",
  label: "settings.provider.preset.lmstudio",
  format: "local",
  defaultBaseUrl: "http://localhost:1234/v1",
});
