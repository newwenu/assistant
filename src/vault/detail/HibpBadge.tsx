import { Typography } from "antd";

interface HibpBadgeProps {
  leaked: boolean;
  count: number;
}

export default function HibpBadge({ leaked, count }: HibpBadgeProps) {
  if (!leaked) return null;

  return (
    <Typography.Text type="danger" style={{ fontSize: 12 }}>
      {count > 0 ? `⚠ Found in ${count} breaches` : "⚠ Found in data breaches"}
    </Typography.Text>
  );
}
