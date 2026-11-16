import { beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { storageService } from "@/services/desktop/storageService";
import { admitAttachment } from "../admit";
import {
  prefetchAttachmentLimits,
  resetAttachmentLimitsForTest,
} from "../limits";
import type { Attachment, VisionSupport } from "../types";
import { ALLOWED_FILE_EXTENSIONS, ALLOWED_IMAGE_EXTENSIONS } from "../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@/services/desktop/storageService", () => ({
  storageService: { getAttachmentLimits: vi.fn() },
}));

const openMock = vi.mocked(open);
const getAttachmentLimits = vi.mocked(storageService.getAttachmentLimits);

async function loadLimits(maxImages = 4, maxFiles = 4) {
  resetAttachmentLimitsForTest();
  getAttachmentLimits.mockResolvedValue({
    maxImages,
    maxFiles,
    maxImageBytes: 8 * 1024 * 1024,
    maxFileBytes: 16 * 1024 * 1024,
  });
  await prefetchAttachmentLimits();
}

beforeEach(async () => {
  await loadLimits();
});

function image(id: string): Attachment {
  return {
    kind: "image",
    id,
    file: new File([new Uint8Array(1)], `${id}.png`, { type: "image/png" }),
    previewUrl: `blob:${id}`,
    originalSizeBytes: 1,
    mimeType: "image/png",
  };
}

function admit(
  kind: Attachment,
  visionSupport: VisionSupport,
  attachments: readonly Attachment[] = [],
) {
  return admitAttachment(kind, { attachments, visionSupport });
}

describe("admitAttachment — vision three states", () => {
  it("allows images when vision is unknown", () => {
    expect(admit(image("a"), "unknown")).toBeNull();
  });

  it("rejects images when vision is unsupported", () => {
    expect(admit(image("a"), "unsupported")).toBe("no_vision");
  });

  it("allows images when vision is supported", () => {
    expect(admit(image("a"), "supported")).toBeNull();
  });

  it("still enforces the image count limit when unknown", () => {
    const existing = ["a", "b", "c", "d"].map(image);
    expect(admit(image("e"), "unknown", existing)).toBe("too_many");
  });

  it("follows the image count reported by the backend DTO", async () => {
    await loadLimits(1, 4);
    expect(admit(image("a"), "supported")).toBeNull();
    expect(admit(image("b"), "supported", [image("a")])).toBe("too_many");
  });

  it("follows the file count reported by the backend DTO", async () => {
    await loadLimits(4, 1);
    const doc = (id: string): Attachment => ({
      kind: "file",
      id,
      file: new File([], `${id}.md`, { type: "text/markdown" }),
      previewUrl: "",
      originalSizeBytes: 1,
      mimeType: "text/markdown",
    });
    expect(admit(doc("a"), "supported")).toBeNull();
    expect(admit(doc("b"), "supported", [doc("a")])).toBe("too_many_files");
  });
});

describe("pickAttachmentPaths — vision three states", () => {
  beforeEach(() => {
    openMock.mockReset();
  });

  async function extensionsFor(visionSupport: VisionSupport) {
    openMock.mockResolvedValue([]);
    const { pickAttachmentPaths } = await import("../sources/filePicker");
    await pickAttachmentPaths(visionSupport);
    return openMock.mock.calls[0][0]?.filters?.[0].extensions ?? [];
  }

  it("offers image extensions when vision is unknown", async () => {
    const extensions = await extensionsFor("unknown");
    for (const ext of ALLOWED_IMAGE_EXTENSIONS) {
      expect(extensions).toContain(ext);
    }
    for (const ext of ALLOWED_FILE_EXTENSIONS) {
      expect(extensions).toContain(ext);
    }
  });

  it("offers only file extensions when vision is unsupported", async () => {
    const extensions = await extensionsFor("unsupported");
    expect(extensions).toEqual([...ALLOWED_FILE_EXTENSIONS]);
  });

  it("offers image extensions when vision is supported", async () => {
    const extensions = await extensionsFor("supported");
    expect(extensions).toContain("png");
  });
});
