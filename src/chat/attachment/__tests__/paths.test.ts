import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));
vi.mock("@tauri-apps/api/path", () => ({
  join: vi.fn((base: string, name: string) =>
    Promise.resolve(`${base}/${name}`),
  ),
}));
vi.mock("@/services/desktop/storageService", () => ({
  storageService: {
    getAssetDirs: vi.fn(),
    openAttachment: vi.fn(),
  },
}));

const { storageService } = await import("@/services/desktop/storageService");
const getAssetDirs = vi.mocked(storageService.getAssetDirs);
const openAttachment = vi.mocked(storageService.openAttachment);

async function loadPaths() {
  vi.resetModules();
  return import("../paths");
}

async function loadMascotAssetUrl() {
  vi.resetModules();
  return import("@/mascot/core/assetUrl");
}

const DIRS = {
  attachmentsDir: "/data/app/attachments",
  mascotAssetsDir: "/data/app/mascot-assets",
};

describe("attachment paths", () => {
  beforeEach(() => {
    getAssetDirs.mockReset();
    getAssetDirs.mockResolvedValue(DIRS);
  });

  it("builds the attachment url from get_asset_dirs", async () => {
    const { attachmentUrl } = await loadPaths();
    await expect(attachmentUrl("abc.png")).resolves.toBe(
      "asset:///data/app/attachments/abc.png",
    );
    expect(getAssetDirs).toHaveBeenCalledTimes(1);
  });

  it("caches the directory lookup across both helpers", async () => {
    vi.resetModules();
    const { attachmentUrl } = await import("../paths");
    const { mascotAssetUrl } = await import("@/mascot/core/assetUrl");
    await attachmentUrl("abc.png");
    await mascotAssetUrl("gif/custom.gif");
    expect(getAssetDirs).toHaveBeenCalledTimes(1);
  });

  it("retries the lookup after a failure instead of caching the error", async () => {
    const { attachmentUrl } = await loadPaths();
    getAssetDirs.mockRejectedValueOnce("not managed");
    await expect(attachmentUrl("abc.png")).rejects.toBe("not managed");
    await expect(attachmentUrl("abc.png")).resolves.toBe(
      "asset:///data/app/attachments/abc.png",
    );
    expect(getAssetDirs).toHaveBeenCalledTimes(2);
  });

  it("opens attachments through the backend without a local path", async () => {
    const { openAttachmentFile } = await loadPaths();
    openAttachment.mockResolvedValue(true);
    await expect(openAttachmentFile("abc.png")).resolves.toBe(true);
    expect(openAttachment).toHaveBeenCalledWith("abc.png");
  });

  it("returns false for an empty attachment id", async () => {
    const { openAttachmentFile } = await loadPaths();
    await expect(openAttachmentFile("")).resolves.toBe(false);
    expect(openAttachment).not.toHaveBeenCalled();
  });
});

describe("mascot asset url", () => {
  beforeEach(() => {
    getAssetDirs.mockReset();
    getAssetDirs.mockResolvedValue(DIRS);
  });

  it("builds the mascot url from get_asset_dirs and appends version", async () => {
    const { mascotAssetUrl } = await loadMascotAssetUrl();
    await expect(mascotAssetUrl("rive/custom.riv")).resolves.toBe(
      "asset:///data/app/mascot-assets/rive/custom.riv",
    );
    await expect(mascotAssetUrl("rive/custom.riv", 1700)).resolves.toBe(
      "asset:///data/app/mascot-assets/rive/custom.riv?v=1700",
    );
  });

  it("recognizes only local asset urls", async () => {
    const { isLocalAssetUrl } = await loadMascotAssetUrl();
    expect(
      isLocalAssetUrl(
        "http://asset.localhost/C%3A%5Cdata%5Cmascot-assets%5Crive%5Ccustom.riv",
      ),
    ).toBe(true);
    expect(isLocalAssetUrl("asset://localhost/data/app/a.riv")).toBe(true);
    expect(isLocalAssetUrl("https://example.com/a.riv")).toBe(false);
    expect(isLocalAssetUrl("/mascot.riv")).toBe(false);
    expect(isLocalAssetUrl("")).toBe(false);
  });

  it("caches the directory lookup across repeated lookups", async () => {
    const { mascotAssetUrl } = await loadMascotAssetUrl();
    await mascotAssetUrl("rive/a.riv");
    await mascotAssetUrl("rive/b.riv", 42);
    expect(getAssetDirs).toHaveBeenCalledTimes(1);
  });
});
