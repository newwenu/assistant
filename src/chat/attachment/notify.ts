import type { AdmitError } from "./types";
import { attachmentLimits } from "./limits";
import { notifyInfo, notifyWarning } from "../../stores/notifyStore";

const MEGABYTE = 1024 * 1024;

export function notifyVisionUnknown(): void {
  notifyInfo(
    "chat.input.model_vision_unknown",
    "chat.input.model_vision_unknown",
  );
}

export function notifyAttachError(error: AdmitError): void {
  const limits = attachmentLimits();
  switch (error) {
    case "too_many":
      notifyWarning(
        "chat.input.too_many_images",
        "chat.input.too_many_images",
        { max: limits.maxImages },
      );
      break;
    case "too_large":
      notifyWarning(
        "chat.input.image_too_large",
        "chat.input.image_too_large",
        { max: limits.maxImageBytes / MEGABYTE },
      );
      break;
    case "invalid_type":
      notifyWarning(
        "chat.input.invalid_image_type",
        "chat.input.invalid_image_type",
      );
      break;
    case "no_vision":
      notifyInfo("chat.input.model_no_vision", "chat.input.model_no_vision");
      break;
    case "too_many_files":
      notifyWarning("chat.input.too_many_files", "chat.input.too_many_files", {
        max: limits.maxFiles,
      });
      break;
    case "file_too_large":
      notifyWarning("chat.input.file_too_large", "chat.input.file_too_large", {
        max: limits.maxFileBytes / MEGABYTE,
      });
      break;
    case "invalid_file_type":
      notifyWarning(
        "chat.input.invalid_file_type",
        "chat.input.invalid_file_type",
      );
      break;
    case "import_failed":
      notifyWarning("chat.input.import_failed", "chat.input.import_failed");
      break;
  }
}
