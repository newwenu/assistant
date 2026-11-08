import { defineAgentKind } from "./agentModeRegistry";

defineAgentKind({
  id: "chat",
  label: "settings.agent_mode.kind.chat",
  color: "blue",
});
defineAgentKind({
  id: "agent",
  label: "settings.agent_mode.kind.agent",
  color: "purple",
});
