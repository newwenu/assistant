import { describe, it, expect } from "vitest";
import { toRecordSnapshot } from "../recordSnapshot";
import { makeMessage } from "../../../test-utils/messageFixtures";
import type { MessageDto } from "../../../types/protocol.contract";

const DTO_KEY_ORDER: Array<keyof MessageDto> = [
  "id",
  "conversationId",
  "role",
  "content",
  "tokens",
  "timestamp",
  "parentId",
  "isPinned",
  "reasoning",
  "siblingIndex",
  "siblingCount",
  "providerId",
  "modelId",
  "modeId",
  "status",
  "toolCalls",
  "turnIndex",
  "iteration",
];

describe("toRecordSnapshot", () => {
  it("keeps keys in MessageDto order", () => {
    const record = toRecordSnapshot(makeMessage("m1"));
    expect(Object.keys(record)).toEqual(DTO_KEY_ORDER);
  });

  it("projects tool calls without local-only fields", () => {
    const record = toRecordSnapshot(
      makeMessage("m1", {
        role: "assistant",
        toolCalls: [
          {
            callId: "c1",
            name: "read_file",
            status: "success",
            arguments: { path: "a.ts" },
            argsText: '{"path":"a.ts"}',
            argsPreview: "a.ts",
            resultText: "ok",
            compact: null,
            startedAt: 1,
            finishedAt: 2,
            approval: null,
            question: null,
            errorKind: null,
            confirmationWaitMs: null,
          },
        ],
      }),
    );
    expect(record.toolCalls).toEqual([
      {
        callId: "c1",
        name: "read_file",
        arguments: { path: "a.ts" },
        status: "success",
        result: "ok",
        compact: null,
      },
    ]);
  });

  it("serializes missing values as null instead of dropping keys", () => {
    const record = toRecordSnapshot(makeMessage("m1"));
    const json = JSON.parse(JSON.stringify(record)) as Record<string, unknown>;
    expect(Object.keys(json)).toHaveLength(DTO_KEY_ORDER.length);
    expect(json.tokens).toBeNull();
    expect(json.reasoning).toBeNull();
    expect(json.siblingCount).toBeNull();
    expect(json.providerId).toBeNull();
    expect(json.modelId).toBeNull();
    expect(json.modeId).toBeNull();
    expect(json.status).toBeNull();
    expect(json.turnIndex).toBeNull();
    expect(json.toolCalls).toEqual([]);
  });

  it("is pure: snapshot shares no mutable state with the message", () => {
    const message = makeMessage("m1", {
      role: "assistant",
      tokens: 12,
      turnIndex: 3,
      iteration: 2,
      status: "completed",
      toolCalls: [
        {
          callId: "c1",
          name: "read_file",
          status: "success",
          arguments: { path: "a.ts" },
          resultText: "ok",
        },
      ],
    });
    const before = toRecordSnapshot(message);
    const after = toRecordSnapshot(message);
    expect(after).toEqual(before);

    const toolCalls = before.toolCalls as Array<Record<string, unknown>>;
    toolCalls[0].name = "mutated";
    expect(message.toolCalls?.[0].name).toBe("read_file");
  });
});
