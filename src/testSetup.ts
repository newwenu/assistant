if (typeof window === "undefined") {
  Object.defineProperty(globalThis, "window", {
    value: {},
    writable: true,
    configurable: true,
  });
}
Object.defineProperty(globalThis, "import.meta", {
  value: { env: { DEV: false, MODE: "test", PROD: true, SSR: false } },
  writable: true,
  configurable: true,
});
