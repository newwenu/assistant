import { beforeEach, describe, expect, it } from "vitest";
import { loadPrefs, pushSendHistory, savePrefs } from "./localPrefs";

const STORAGE_KEY = "serial_debug.prefs";

let store: Map<string, string>;

beforeEach(() => {
  store = new Map();
  Object.defineProperty(window, "localStorage", {
    value: {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
      removeItem: (key: string) => {
        store.delete(key);
      },
    },
    writable: true,
    configurable: true,
  });
});

function writeRaw(value: unknown): void {
  store.set(STORAGE_KEY, JSON.stringify(value));
}

describe("loadPrefs", () => {
  it("空存储回落默认值，收发视图均为 text", () => {
    const prefs = loadPrefs();
    expect(prefs.sendMode).toBe("text");
    expect(prefs.receiveView).toBe("text");
    expect(prefs.baudRate).toBe(115200);
    expect(prefs.encoding).toBe("utf8");
    expect(prefs.lineEnding).toBe("none");
    expect(prefs.timestamp).toBe(false);
    expect(prefs.history).toEqual([]);
  });

  it("损坏的 JSON 回落默认值", () => {
    store.set(STORAGE_KEY, "{not json");
    expect(loadPrefs().sendMode).toBe("text");
    expect(loadPrefs().receiveView).toBe("text");
  });

  it("非对象根节点回落默认值", () => {
    writeRaw([1, 2, 3]);
    expect(loadPrefs().sendMode).toBe("text");
  });

  it("收发视图的非法取值回落 text", () => {
    writeRaw({ sendMode: "HEX", receiveView: 1 });
    const prefs = loadPrefs();
    expect(prefs.sendMode).toBe("text");
    expect(prefs.receiveView).toBe("text");
  });

  it("收发视图的合法取值被保留", () => {
    writeRaw({ sendMode: "hex", receiveView: "hex" });
    const prefs = loadPrefs();
    expect(prefs.sendMode).toBe("hex");
    expect(prefs.receiveView).toBe("hex");
  });
});

describe("savePrefs", () => {
  it("收发视图落盘后可读回", () => {
    savePrefs({ sendMode: "hex", receiveView: "hex" });
    const prefs = loadPrefs();
    expect(prefs.sendMode).toBe("hex");
    expect(prefs.receiveView).toBe("hex");
  });

  it("局部更新不覆盖其它字段", () => {
    savePrefs({ portName: "COM5", baudRate: 9600 });
    const prefs = savePrefs({ receiveView: "hex" });
    expect(prefs.portName).toBe("COM5");
    expect(prefs.baudRate).toBe(9600);
    expect(prefs.receiveView).toBe("hex");
  });
});

describe("pushSendHistory", () => {
  it("只保留最近 20 条", () => {
    for (let index = 1; index <= 25; index += 1) {
      pushSendHistory({ mode: "hex", text: String(index), lineEnding: "none" });
    }
    const history = loadPrefs().history;
    expect(history).toHaveLength(20);
    expect(history[0]?.text).toBe("6");
    expect(history[19]?.text).toBe("25");
  });

  it("过滤掉结构非法的历史条目", () => {
    writeRaw({
      history: [
        { mode: "hex", text: "20 01", lineEnding: "none" },
        { mode: "ascii", text: "20 01", lineEnding: "none" },
        { mode: "text", text: 1, lineEnding: "none" },
        { mode: "text", text: "hi", lineEnding: "unknown" },
      ],
    });
    const history = loadPrefs().history;
    expect(history).toHaveLength(1);
    expect(history[0]?.text).toBe("20 01");
  });
});
