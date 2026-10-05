import React, { useState } from "react";
import { Modal, Input, Form } from "antd";

export interface PromptTemplate {
  id: string;
  name: string;
  category: string;
  content: string;
  variables_json: string | null;
  is_builtin: boolean;
}

interface TemplateCardsProps {
  templates: PromptTemplate[];
  onSelect: (content: string) => void;
}

const CATEGORY_ICONS: Record<string, string> = {
  text: "📝",
  dev: "💻",
  daily: "💡",
};

function extractVariables(content: string): string[] {
  const matches = content.match(/\{\{(\w+)\}\}/g);
  if (!matches) return [];
  return [...new Set(matches.map((m) => m.slice(2, -2)))];
}

function fillVariables(
  content: string,
  values: Record<string, string>,
): string {
  let result = content;
  for (const [key, value] of Object.entries(values)) {
    result = result.replace(
      new RegExp(`\\{\\{${key}\\}\\}`, "g"),
      value || `{{${key}}}`,
    );
  }
  return result;
}

export const TemplateCards: React.FC<TemplateCardsProps> = ({
  templates,
  onSelect,
}) => {
  const [modalOpen, setModalOpen] = useState(false);
  const [activeTemplate, setActiveTemplate] = useState<PromptTemplate | null>(
    null,
  );
  const [form] = Form.useForm();

  const handleClick = (template: PromptTemplate) => {
    const vars = extractVariables(template.content);
    if (vars.length === 0) {
      onSelect(template.content);
      return;
    }
    setActiveTemplate(template);
    form.resetFields();
    setModalOpen(true);
  };

  const handleOk = () => {
    if (!activeTemplate) return;
    const values = form.getFieldsValue();
    const filled = fillVariables(activeTemplate.content, values);
    onSelect(filled);
    setModalOpen(false);
    setActiveTemplate(null);
  };

  return (
    <>
      <div className="template-cards">
        {templates.map((t) => (
          <div
            key={t.id}
            className="template-card"
            onClick={() => handleClick(t)}
          >
            <div className="template-card-icon">
              {CATEGORY_ICONS[t.category] || "📄"}
            </div>
            <div className="template-card-name">{t.name}</div>
            <div className="template-card-category">{t.category}</div>
          </div>
        ))}
      </div>
      <Modal
        open={modalOpen}
        title={activeTemplate?.name || "填写模板变量"}
        onOk={handleOk}
        onCancel={() => {
          setModalOpen(false);
          setActiveTemplate(null);
        }}
        okText="使用"
        cancelText="取消"
      >
        <Form form={form} layout="vertical">
          {activeTemplate &&
            extractVariables(activeTemplate.content).map((v) => (
              <Form.Item key={v} name={v} label={v}>
                <Input placeholder={`请输入 ${v}`} />
              </Form.Item>
            ))}
        </Form>
      </Modal>
    </>
  );
};
