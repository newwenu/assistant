import type { MascotFace } from "../../core/types";
import { REST_SMILE_PATH } from "./visemes";

/** 非说话态嘴形：按表情给出固定路径，说话态由 viseme 覆盖。 */
export function restMouthPath(face: MascotFace): string {
  switch (face) {
    case "sleep":
      return "M496,588 Q520,593 544,588 Q520,592 496,588 Z";
    case "happy":
    case "celebrating":
    case "dancing":
    case "waving":
      return "M460,565 Q520,635 580,565 Q520,605 460,565 Z";
    case "concerned":
      return "M478,605 Q520,560 562,605 Q520,590 478,605 Z";
    case "confused":
      return "M478,580 Q520,610 562,575 Q520,597 478,580 Z";
    case "thinking":
    case "writing":
    case "reading":
    case "working":
      return "M488,585 Q520,595 552,585 Q520,592 488,585 Z";
    case "listening":
      return "M495,580 Q520,600 545,580 Q520,615 495,580 Z";
    case "curious":
      return "M500,578 Q520,598 540,578 Q520,610 500,578 Z";
    case "proud":
      return "M468,570 Q520,625 572,570 Q520,600 468,570 Z";
    case "cautious":
      return "M482,600 Q520,568 558,600 Q520,585 482,600 Z";
    default:
      return REST_SMILE_PATH;
  }
}
