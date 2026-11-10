import type { AdmitError } from "./types";
import {
  ATTACHMENT_MAX_FILES,
  ATTACHMENT_MAX_FILE_SIZE_BYTES,
  ATTACHMENT_MAX_IMAGES,
  ATTACHMENT_MAX_IMAGE_SIZE_BYTES,
} from "./types";
import { notifyInfo, notifyWarning } from "../../stores/notifyStore";

export function notifyAttachError(error: AdmitError): void {
  switch (error) {
    case "too_many":
      notifyWarning(
        "chat.input.too_many_images",
        "chat.input.too_many_images",
        { max: ATTACHMENT_MAX_IMAGES },
      );
      break;
    case "too_large":
      notifyWarning(
        "chat.input.image_too_large",
        "chat.input.image_too_large",
        { max: ATTACHMENT_MAX_IMAGE_SIZE_BYTES / (1024 * 1024) },
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
        max: ATTACHMENT_MAX_FILES,
      });
      break;
    case "file_too_large":
      notifyWarning("chat.input.file_too_large", "chat.input.file_too_large", {
        max: ATTACHMENT_MAX_FILE_SIZE_BYTES / (1024 * 1024),
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
