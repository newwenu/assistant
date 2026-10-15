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
  max_tokens: 4096,
  reasoning_enabled: null,
  reasoning_effort: null,
});

defineBuiltinModeDefaults("builtin-quick", {
  prompt: "",
  temperature: 0.3,
  max_tokens: 2048,
  reasoning_enabled: false,
  reasoning_effort: null,
});

defineBuiltinModeDefaults("builtin-deep", {
  prompt: "",
  temperature: 0.7,
  max_tokens: 8192,
  reasoning_enabled: true,
  reasoning_effort: "high",
});
