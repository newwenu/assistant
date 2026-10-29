import { evaluate } from "mathjs";

export function safeEvaluate(expression: string): string {
  try {
    if (!expression.trim()) return "";
    return String(evaluate(expression));
  } catch {
    return "";
  }
}
