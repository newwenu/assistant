import { Input, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type { PaperErrorPosDto } from "../core/vault.types";
import {
  PAPER_CHARS,
  formatPaperKey,
  groupPaperKey,
  normalizePaperInput,
} from "../core/paperKey";

interface RecoveryKeyFormProps {
  value: string;
  onChange: (normalized: string) => void;
  errorPos?: PaperErrorPosDto | null;
  disabled?: boolean;
  autoFocus?: boolean;
}

export default function RecoveryKeyForm({
  value,
  onChange,
  errorPos,
  disabled,
  autoFocus,
}: RecoveryKeyFormProps) {
  const { t } = useTranslation();
  const groups = groupPaperKey(value);
  const filled = value.length;

  return (
    <div>
      <Input
        size="large"
        value={formatPaperKey(value)}
        onChange={(e) => onChange(normalizePaperInput(e.target.value))}
        disabled={disabled}
        autoFocus={autoFocus}
        spellCheck={false}
        autoComplete="off"
        style={{ fontFamily: "monospace", letterSpacing: 1 }}
        placeholder={t("vault.recovery.paper.inputLabel")}
        aria-label={t("vault.recovery.paper.inputLabel")}
      />
      <Typography.Text
        type="secondary"
        style={{ display: "block", marginTop: 8, fontSize: 12 }}
      >
        {t("vault.recovery.paper.inputHint")}
      </Typography.Text>
      <div
        style={{
          display: "flex",
          gap: 8,
          marginTop: 12,
          justifyContent: "center",
        }}
      >
        {Array.from({ length: 5 }, (_, groupIndex) => {
          const chars = groups[groupIndex] ?? "";
          const groupNo = groupIndex + 1;
          const groupHasError =
            errorPos !== null &&
            errorPos !== undefined &&
            errorPos.group === groupNo;
          return (
            <div
              key={groupNo}
              style={{
                display: "flex",
                gap: 2,
                padding: 4,
                borderRadius: 6,
                border: groupHasError
                  ? "1px solid var(--ant-color-error)"
                  : "1px solid var(--ant-color-border)",
              }}
            >
              {Array.from({ length: 8 }, (_, charIndex) => {
                const char = chars[charIndex] ?? "";
                const charNo = charIndex + 1;
                const charHasError =
                  groupHasError &&
                  errorPos !== null &&
                  errorPos !== undefined &&
                  errorPos.index === charNo;
                return (
                  <span
                    key={charNo}
                    style={{
                      width: 14,
                      textAlign: "center",
                      fontFamily: "monospace",
                      fontWeight: charHasError ? 700 : 400,
                      color: charHasError
                        ? "var(--ant-color-error)"
                        : char
                          ? "inherit"
                          : "var(--ant-color-text-quaternary)",
                    }}
                  >
                    {char || "_"}
                  </span>
                );
              })}
            </div>
          );
        })}
      </div>
      <Typography.Text
        type="secondary"
        style={{
          display: "block",
          textAlign: "center",
          marginTop: 8,
          fontSize: 12,
        }}
      >
        {filled}/{PAPER_CHARS}
      </Typography.Text>
    </div>
  );
}
