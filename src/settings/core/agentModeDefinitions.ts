import {
  defineAgentKind,
  defineBuiltinModeDefaults,
} from "./agentModeRegistry";

defineAgentKind({ id: "default", label: "agent_kind.default", color: "blue" });
defineAgentKind({ id: "quick", label: "agent_kind.quick", color: "green" });
defineAgentKind({ id: "deep", label: "agent_kind.deep", color: "purple" });
defineAgentKind({ id: "custom", label: "agent_kind.custom", color: "orange" });

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
