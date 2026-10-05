import React, { useEffect, useState } from "react";
import { Card, Tag, Typography, Space } from "antd";
import { invoke } from "@tauri-apps/api/core";

interface PromptTemplate {
  id: string;
  name: string;
  category: string;
  content: string;
  variables_json: string | null;
  is_builtin: boolean;
}

export const TemplateManager: React.FC = () => {
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);

  useEffect(() => {
    invoke("list_prompt_templates").then((result) => {
      setTemplates((result as PromptTemplate[]) || []);
    });
  }, []);

  return (
    <div className="template-manager">
      <Typography.Title level={4}>Prompt 模板</Typography.Title>
      <div className="template-manager-grid">
        {templates.map((t) => (
          <Card key={t.id} size="small" className="template-manager-card">
            <div className="template-manager-card-header">
              <Typography.Text strong>{t.name}</Typography.Text>
              <Space>
                <Tag>{t.category}</Tag>
                {t.is_builtin && <Tag color="blue">内置</Tag>}
              </Space>
            </div>
            <Typography.Paragraph
              type="secondary"
              ellipsis={{ rows: 3 }}
              style={{ fontSize: 12, marginTop: 8 }}
            >
              {t.content}
            </Typography.Paragraph>
            {t.variables_json && (
              <div style={{ marginTop: 4 }}>
                {(() => {
                  try {
                    const vars = JSON.parse(t.variables_json) as string[];
                    return vars.map((v) => (
                      <Tag key={v} style={{ fontSize: 11 }}>
                        {`{{${v}}}`}
                      </Tag>
                    ));
                  } catch {
                    return null;
                  }
                })()}
              </div>
            )}
          </Card>
        ))}
      </div>
    </div>
  );
};
