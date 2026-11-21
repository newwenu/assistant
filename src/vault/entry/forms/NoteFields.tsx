import { Form, Input } from "antd";
import { useTranslation } from "react-i18next";

interface NoteFieldsProps {
  notes: string;
  onNotesChange: (value: string) => void;
  hasNotes?: boolean;
}

export default function NoteFields({
  notes,
  onNotesChange,
  hasNotes = false,
}: NoteFieldsProps) {
  const { t } = useTranslation();

  return (
    <Form.Item label={t("vault.entry.notes")}>
      <Input.TextArea
        rows={3}
        value={notes}
        onChange={(e) => onNotesChange(e.target.value)}
        placeholder={
          hasNotes && !notes ? t("vault.entry.notes_set_hint") : undefined
        }
      />
    </Form.Item>
  );
}
