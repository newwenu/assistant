import { describe, expect, it } from "vitest";
import {
  createFileAttachmentFromFile,
  createFileAttachmentFromPath,
  createImageAttachmentFromFile,
  createImageAttachmentFromPath,
  validateImageFile,
} from "../factory";
import { ATTACHMENT_MAX_IMAGE_SIZE_BYTES } from "../types";

function imageFile(name = "shot.png", type = "image/png", size = 0): File {
  return new File([new Uint8Array(size)], name, { type });
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
        imageFile("big.png", "image/png", ATTACHMENT_MAX_IMAGE_SIZE_BYTES + 1),
      ),
    ).toBe("too_large");
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

  it("createImageAttachmentFromPath rejects unknown extension", () => {
    const result = createImageAttachmentFromPath("C:/tmp/report.pdf");
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

  it("createFileAttachmentFromPath rejects unknown extension", () => {
    const result = createFileAttachmentFromPath("C:/tmp/binary.exe");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("invalid_file_type");
  });
});
