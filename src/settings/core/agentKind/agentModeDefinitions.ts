import {
  defineAgentKind,
  defineBuiltinModeDefaults,
} from "./agentModeRegistry";

defineAgentKind({
  id: "default",
  label: "settings.agent_mode.kind.default",
  color: "blue",
});
defineAgentKind({
  id: "quick",
  label: "settings.agent_mode.kind.quick",
  color: "green",
});
defineAgentKind({
  id: "deep",
  label: "settings.agent_mode.kind.deep",
  color: "purple",
});
defineAgentKind({
  id: "custom",
  label: "settings.agent_mode.kind.custom",
  color: "orange",
});

defineBuiltinModeDefaults("builtin-default", {
  prompt: "",
  temperature: 0.7,
  maxTokens: 4096,
  reasoningEnabled: null,
  reasoningEffort: null,
});

defineBuiltinModeDefaults("builtin-quick", {
  prompt: "",
  temperature: 0.3,
  maxTokens: 2048,
  reasoningEnabled: false,
  reasoningEffort: null,
});

defineBuiltinModeDefaults("builtin-deep", {
  prompt: "",
  temperature: 0.7,
  maxTokens: 8192,
  reasoningEnabled: true,
  reasoningEffort: "high",
});
