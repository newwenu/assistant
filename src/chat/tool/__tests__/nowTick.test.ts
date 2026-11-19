import { beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeNowTick } from "../useNowTick";

type IntervalHandler = () => void;

let handlers: IntervalHandler[] = [];
let nextId = 0;

const setIntervalStub = vi.fn((handler: IntervalHandler) => {
  handlers.push(handler);
  nextId += 1;
  return nextId;
});
const clearIntervalStub = vi.fn();

beforeEach(() => {
  handlers = [];
  nextId = 0;
  setIntervalStub.mockClear();
  clearIntervalStub.mockClear();
  Object.defineProperty(window, "setInterval", {
    value: setIntervalStub,
    writable: true,
    configurable: true,
  });
  Object.defineProperty(window, "clearInterval", {
    value: clearIntervalStub,
    writable: true,
    configurable: true,
  });
});

describe("subscribeNowTick 共享 ticker（G3）", () => {
  it("多个订阅者只创建一个 setInterval，且同一次 tick 通知全部订阅者", () => {
    const first = vi.fn();
    const second = vi.fn();
    const offFirst = subscribeNowTick(first);
    const offSecond = subscribeNowTick(second);

    expect(setIntervalStub).toHaveBeenCalledTimes(1);

    handlers[0]();
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);

    offFirst();
    offSecond();
  });

  it("最后一个退订才清理定时器，再订阅时重新启动", () => {
    const offFirst = subscribeNowTick(vi.fn());
    const offSecond = subscribeNowTick(vi.fn());

    offFirst();
    expect(clearIntervalStub).not.toHaveBeenCalled();

    offSecond();
    expect(clearIntervalStub).toHaveBeenCalledTimes(1);

    const offAgain = subscribeNowTick(vi.fn());
    expect(setIntervalStub).toHaveBeenCalledTimes(2);
    offAgain();
    expect(clearIntervalStub).toHaveBeenCalledTimes(2);
  });
});
