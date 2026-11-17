import { describe, expect, it } from "vitest";

import type { RiveManifestDto } from "@/types/protocol.contract";

import { asFace } from "../../../core/types";
import {
  EMPTY_RIVE_SCHEMA,
  FALLBACK_RIVE_BINDINGS,
  type RiveSchema,
  deriveFaceToPose,
  filterFaceToPose,
  filterSceneToPose,
  resolveRiveBindings,
} from "../riveIntrospect";
import {
  BUILTIN_RIVE_STATE_ENGINE,
  buildStateEngine,
  resolveScenePose,
} from "../riveStateEngine";

const BUILTIN_SCHEMA: RiveSchema = {
  stateMachines: ["Main"],
  properties: [
    { name: "pose", type: "enumType", enumName: "Pose" },
    { name: "mouthVisemeCode", type: "enumType", enumName: "Viseme" },
    { name: "primaryColor", type: "color", enumName: null },
    { name: "secondaryColor", type: "color", enumName: null },
  ],
  enumValues: {
    pose: ["idle", "thinking"],
    mouthVisemeCode: ["sil", "pp"],
  },
};

const ALIEN_SCHEMA: RiveSchema = {
  stateMachines: ["Blink"],
  properties: [
    { name: "mood", type: "enumType", enumName: "Mood" },
    { name: "lipSync", type: "enumType", enumName: "LipSync" },
    { name: "baseColor", type: "color", enumName: null },
    { name: "accentColor", type: "color", enumName: null },
  ],
  enumValues: {
    mood: ["Idle", "Sad"],
    lipSync: ["sil", "pp"],
  },
};

describe("resolveRiveBindings", () => {
  it("空 schema 回退内置属性名", () => {
    expect(resolveRiveBindings(EMPTY_RIVE_SCHEMA)).toEqual(
      FALLBACK_RIVE_BINDINGS,
    );
  });

  it("内置素材仍绑定原属性名", () => {
    expect(resolveRiveBindings(BUILTIN_SCHEMA)).toEqual({
      pose: "pose",
      viseme: "mouthVisemeCode",
      primaryColor: "primaryColor",
      secondaryColor: "secondaryColor",
    });
  });

  it("按候选名与枚举值启发式匹配自定义命名", () => {
    expect(resolveRiveBindings(ALIEN_SCHEMA)).toEqual({
      pose: "mood",
      viseme: "lipSync",
      primaryColor: "baseColor",
      secondaryColor: "accentColor",
    });
  });

  it("manifest 声明优先于启发式", () => {
    const manifest: RiveManifestDto = {
      properties: {
        pose: "mood",
        viseme: "lipSync",
        primaryColor: "accentColor",
        secondaryColor: "baseColor",
      },
    };
    expect(resolveRiveBindings(ALIEN_SCHEMA, manifest)).toEqual({
      pose: "mood",
      viseme: "lipSync",
      primaryColor: "accentColor",
      secondaryColor: "baseColor",
    });
  });

  it("声明大小写不一致时归一化到实际属性名", () => {
    const manifest: RiveManifestDto = {
      properties: { viseme: "lipsync", primaryColor: "BASECOLOR" },
    };
    const bindings = resolveRiveBindings(ALIEN_SCHEMA, manifest);
    expect(bindings.viseme).toBe("lipSync");
    expect(bindings.primaryColor).toBe("baseColor");
  });

  it("声明路径不在自省结果中时仍按声明使用", () => {
    const manifest: RiveManifestDto = {
      properties: { pose: "group/pose" },
    };
    expect(resolveRiveBindings(ALIEN_SCHEMA, manifest).pose).toBe("group/pose");
  });
});

describe("deriveFaceToPose", () => {
  const base = BUILTIN_RIVE_STATE_ENGINE.states;

  it("按内置映射优先、其次同名、最后模糊匹配", () => {
    const derived = deriveFaceToPose(
      ["Idle", "Happy", "Thinking", "BookReading"],
      base,
    );
    expect(derived.happy).toBe("Happy");
    expect(derived.thinking).toBe("Thinking");
    expect(derived.reading).toBe("BookReading");
    expect(derived.speaking).toBe("Idle");
  });

  it("无 idle 枚举值时回退首个 pose", () => {
    const derived = deriveFaceToPose(["Calm", "Sad"], base);
    expect(derived.idle).toBe("Calm");
    expect(derived.sleep).toBe("Calm");
  });

  it("空 pose 列表时原样返回兜底表", () => {
    expect(deriveFaceToPose([], base)).toEqual(base);
  });

  it("过短的 pose 名不参与模糊匹配", () => {
    const derived = deriveFaceToPose(["A", "Idle"], base);
    expect(derived.dancing).toBe("Idle");
  });
});

describe("filterFaceToPose", () => {
  it("丢弃枚举中不存在的目标值并大小写归一", () => {
    const { kept, dropped } = filterFaceToPose(
      { happy: "happy", sad: "nope" },
      ["Happy", "Idle"],
    );
    expect(kept).toEqual({ happy: "Happy" });
    expect(dropped).toEqual(["sad -> nope"]);
  });
});

describe("filterSceneToPose", () => {
  it("与表情映射共用同一套 pose 校验", () => {
    const { kept, dropped } = filterSceneToPose(
      { ackDone: "happy", streamEnd: "missing" },
      ["Happy", "Idle"],
    );
    expect(kept).toEqual({ ackDone: "Happy" });
    expect(dropped).toEqual(["streamEnd -> missing"]);
  });
});

describe("buildStateEngine", () => {
  it("无参时返回内置引擎", () => {
    expect(buildStateEngine()).toBe(BUILTIN_RIVE_STATE_ENGINE);
  });

  it("按自省出的 pose 与 viseme 构建", () => {
    const engine = buildStateEngine(undefined, {
      poses: ["Idle", "Happy", "Thinking"],
      visemes: ["sil", "pp", "aa"],
    });
    expect(engine.states.idle).toBe("Idle");
    expect(engine.visemeCodes).toEqual(["sil", "pp", "aa"]);
    expect(engine.idlePoseCycle).toEqual(["Happy", "Thinking"]);
  });

  it("manifest 的 faceToPose 覆盖推导结果且丢弃非法值", () => {
    const engine = buildStateEngine(undefined, {
      poses: ["Idle", "Happy"],
      faceToPose: { happy: "Idle", unknown: "Nope" },
    });
    expect(engine.states.happy).toBe("Idle");
    expect(engine.states.unknown).toBeUndefined();
  });

  it("旧签名仍直接覆盖内置映射并保留内置轮转", () => {
    const engine = buildStateEngine({ happy: "celebration" });
    expect(engine.states.happy).toBe("celebration");
    expect(engine.idlePoseCycle).toEqual(
      BUILTIN_RIVE_STATE_ENGINE.idlePoseCycle,
    );
  });

  it("manifest 的 sceneToPose 独立于表情映射且丢弃非法值", () => {
    const engine = buildStateEngine(undefined, {
      poses: ["Idle", "Happy", "Sad"],
      sceneToPose: { ackDone: "Happy", streamEnd: "Nope", idle: "Idle" },
    });
    expect(engine.sceneStates).toEqual({ ackDone: "Happy", idle: "Idle" });
    expect(engine.states.idle).toBe("Idle");
  });
});

describe("resolveScenePose", () => {
  const engine = buildStateEngine(undefined, {
    poses: ["Idle", "Happy", "Sad"],
    faceToPose: { happy: "Happy" },
    sceneToPose: { ackDone: "Sad" },
  });

  it("场景命中时优先于表情映射", () => {
    expect(resolveScenePose("ackDone", asFace("happy"), engine)).toBe("Sad");
  });

  it("场景未声明时回退到表情映射", () => {
    expect(resolveScenePose("streamStart", asFace("happy"), engine)).toBe(
      "Happy",
    );
  });

  it("无场景时按表情映射兜底", () => {
    expect(resolveScenePose(null, asFace("happy"), engine)).toBe("Happy");
  });
});
