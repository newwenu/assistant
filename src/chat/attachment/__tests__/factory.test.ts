import { beforeEach, describe, expect, it, vi } from "vitest";
import { storageService } from "@/services/desktop/storageService";
import type { AttachmentLimitsDto } from "@/types/protocol.contract";
import {
  createFileAttachmentFromFile,
  createImageAttachmentFromFile,
  createPathAttachment,
  validateImageFile,
} from "../factory";
import {
  prefetchAttachmentLimits,
  resetAttachmentLimitsForTest,
} from "../limits";

const LIMITS: AttachmentLimitsDto = {
  maxImages: 4,
  maxFiles: 4,
  maxImageBytes: 8 * 1024 * 1024,
  maxFileBytes: 16 * 1024 * 1024,
};

vi.mock("@/services/desktop/storageService", () => ({
  storageService: { importAttachment: vi.fn(), getAttachmentLimits: vi.fn() },
}));
vi.mock("../paths", () => ({
  attachmentUrl: vi.fn(() => Promise.resolve("asset://localhost/attachment")),
}));

const importAttachment = vi.mocked(storageService.importAttachment);
const getAttachmentLimits = vi.mocked(storageService.getAttachmentLimits);

async function loadLimits(overrides: Partial<AttachmentLimitsDto> = {}) {
  resetAttachmentLimitsForTest();
  getAttachmentLimits.mockResolvedValue({ ...LIMITS, ...overrides });
  await prefetchAttachmentLimits();
}

beforeEach(async () => {
  await loadLimits();
});

function imageFile(name = "shot.png", type = "image/png", size = 0): File {
  return new File([new Uint8Array(size)], name, { type });
}

function imported(name: string, mediaType: string, bytes = 42) {
  return {
    attachmentId: `sha-${name}`,
    mediaType,
    bytes,
    fileName: name,
  };
}

describe("validateImageFile", () => {
  it("rejects non-image MIME type", () => {
    expect(validateImageFile(imageFile("doc.pdf", "application/pdf"))).toBe(
      "invalid_type",
    );
  });

  it("accepts valid image MIME type", () => {
    expect(validateImageFile(imageFile())).toBeNull();
  });

  it("rejects oversized image", () => {
    expect(
      validateImageFile(
        imageFile("big.png", "image/png", LIMITS.maxImageBytes + 1),
      ),
    ).toBe("too_large");
  });

  it("follows the limits reported by the backend DTO", async () => {
    await loadLimits({ maxImageBytes: 10 });
    expect(validateImageFile(imageFile("a.png", "image/png", 11))).toBe(
      "too_large",
    );
    expect(validateImageFile(imageFile("b.png", "image/png", 10))).toBeNull();
  });
});

describe("image factories", () => {
  it("createImageAttachmentFromFile returns image attachment", () => {
    const result = createImageAttachmentFromFile(imageFile());
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.attachment.kind).toBe("image");
  });

  it("createImageAttachmentFromFile rejects invalid type", () => {
    const result = createImageAttachmentFromFile(
      imageFile("doc.pdf", "application/pdf"),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("invalid_type");
  });
});

describe("file factories", () => {
  it("createFileAttachmentFromFile returns file attachment", () => {
    const result = createFileAttachmentFromFile(
      new File([], "notes.md", { type: "text/markdown" }),
    );
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.attachment.kind).toBe("file");
  });

  it("createFileAttachmentFromFile rejects invalid extension", () => {
    const result = createFileAttachmentFromFile(
      new File([], "binary.exe", { type: "application/octet-stream" }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("invalid_file_type");
  });
});

describe("createPathAttachment", () => {
  beforeEach(() => {
    importAttachment.mockReset();
  });

  it("rejects unknown extension before touching the backend", async () => {
    const result = await createPathAttachment("C:/tmp/binary.exe");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("invalid_file_type");
    expect(importAttachment).not.toHaveBeenCalled();
  });

  it("imports an image and keeps only the attachment id", async () => {
    importAttachment.mockResolvedValue(imported("shot.png", "image/png", 128));
    const result = await createPathAttachment("C:/tmp/shot.png");
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.attachment.kind).toBe("image");
      expect(result.attachment.attachmentId).toBe("sha-shot.png");
      expect(result.attachment.originalSizeBytes).toBe(128);
      expect(result.attachment.previewUrl).toBe("asset://localhost/attachment");
      expect(result.attachment).not.toHaveProperty("filePath");
    }
    expect(importAttachment).toHaveBeenCalledWith("C:/tmp/shot.png");
  });

  it("imports a document as a file attachment", async () => {
    importAttachment.mockResolvedValue(imported("notes.md", "text/markdown"));
    const result = await createPathAttachment("C:/tmp/notes.md");
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.attachment.kind).toBe("file");
      expect(result.attachment.attachmentId).toBe("sha-notes.md");
      expect(result.attachment.previewUrl).toBe("");
    }
  });

  it("maps backend too_large to the admit error", async () => {
    importAttachment.mockRejectedValue("too_large");
    const result = await createPathAttachment("C:/tmp/shot.png");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("too_large");
  });

  it("maps backend file_too_large to the admit error", async () => {
    importAttachment.mockRejectedValue("file_too_large");
    const result = await createPathAttachment("C:/tmp/notes.md");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("file_too_large");
  });

  it("falls back to import_failed for unexpected backend errors", async () => {
    importAttachment.mockRejectedValue("read failed: permission denied");
    const result = await createPathAttachment("C:/tmp/shot.png");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("import_failed");
  });
});
