import { defineProviderFormat } from "./providerFormatRegistry";
import { defineProviderPreset } from "./providerPresetRegistry";

defineProviderFormat({
  id: "openai",
  label: "provider.format.openai",
  description: "provider.format.openai.description",
  needsApiKey: true,
  keyPlaceholder: "sk-...",
  color: "var(--color-primary)",
});

defineProviderFormat({
  id: "anthropic",
  label: "provider.format.anthropic",
  description: "provider.format.anthropic.description",
  needsApiKey: true,
  keyPlaceholder: "sk-ant-...",
  color: "var(--color-format-anthropic)",
});

defineProviderFormat({
  id: "local",
  label: "provider.format.local",
  description: "provider.format.local.description",
  needsApiKey: false,
  color: "var(--color-success)",
});

defineProviderPreset({
  slug: "openai",
  label: "provider.preset.openai",
  format: "openai",
  defaultBaseUrl: "https://api.openai.com/v1",
});

defineProviderPreset({
  slug: "deepseek",
  label: "provider.preset.deepseek",
  format: "openai",
  defaultBaseUrl: "https://api.deepseek.com/v1",
});

defineProviderPreset({
  slug: "groq",
  label: "provider.preset.groq",
  format: "openai",
  defaultBaseUrl: "https://api.groq.com/openai/v1",
});

defineProviderPreset({
  slug: "openrouter",
  label: "provider.preset.openrouter",
  format: "openai",
  defaultBaseUrl: "https://openrouter.ai/api/v1",
});

defineProviderPreset({
  slug: "mistral",
  label: "provider.preset.mistral",
  format: "openai",
  defaultBaseUrl: "https://api.mistral.ai/v1",
});

defineProviderPreset({
  slug: "zhipu",
  label: "provider.preset.zhipu",
  format: "openai",
  defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
});

defineProviderPreset({
  slug: "qwen",
  label: "provider.preset.qwen",
  format: "openai",
  defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
});

defineProviderPreset({
  slug: "anthropic",
  label: "provider.preset.anthropic",
  format: "anthropic",
  defaultBaseUrl: "https://api.anthropic.com/v1",
});

defineProviderPreset({
  slug: "ollama",
  label: "provider.preset.ollama",
  format: "local",
  defaultBaseUrl: "http://localhost:11434/v1",
});

defineProviderPreset({
  slug: "lmstudio",
  label: "provider.preset.lmstudio",
  format: "local",
  defaultBaseUrl: "http://localhost:1234/v1",
});
