import { useState, useEffect } from "react";
import { Input, Typography, Button, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const m = hex
    .replace("#", "")
    .match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return null;
  return {
    r: parseInt(m[1], 16),
    g: parseInt(m[2], 16),
    b: parseInt(m[3], 16),
  };
}

function rgbToHsl(
  r: number,
  g: number,
  b: number,
): { h: number; s: number; l: number } {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b),
    min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return { h: 0, s: 0, l: Math.round(l * 100) };
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;
  return {
    h: Math.round(h * 360),
    s: Math.round(s * 100),
    l: Math.round(l * 100),
  };
}

export default function ColorConverter() {
  const [hex, setHex] = useState("#1ea54c");
  const [rgb, setRgb] = useState("");
  const [hsl, setHsl] = useState("");
  const [rgba, setRgba] = useState("");
  const [alpha] = useState(1);

  useEffect(() => {
    const c = hexToRgb(hex);
    if (!c) return;
    setRgb(`rgb(${c.r}, ${c.g}, ${c.b})`);
    setRgba(`rgba(${c.r}, ${c.g}, ${c.b}, ${alpha})`);
    const hslVal = rgbToHsl(c.r, c.g, c.b);
    setHsl(`hsl(${hslVal.h}, ${hslVal.s}%, ${hslVal.l}%)`);
  }, [hex, alpha]);

  const copy = (value: string) => {
    navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  const formats = [
    { label: "HEX", value: hex },
    { label: "RGB", value: rgb },
    { label: "HSL", value: hsl },
    { label: "RGBA", value: rgba },
  ];

  return (
    <div>
      <div
        style={{
          display: "flex",
          gap: 12,
          marginBottom: 16,
          alignItems: "center",
        }}
      >
        <input
          type="color"
          value={hex}
          onChange={(e) => setHex(e.target.value)}
          style={{ width: 48, height: 48, border: "none", cursor: "pointer" }}
        />
        <Input
          value={hex}
          onChange={(e) => setHex(e.target.value)}
          placeholder="#1ea54c"
          style={{ flex: 1, fontFamily: "monospace" }}
        />
      </div>
      {formats.map((f) => (
        <div
          key={f.label}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 8,
          }}
        >
          <Typography.Text strong style={{ flex: "0 0 50px", fontSize: 13 }}>
            {f.label}
          </Typography.Text>
          <Input
            value={f.value}
            readOnly
            size="small"
            style={{ flex: 1, fontFamily: "monospace" }}
          />
          <Button
            icon={<CopyOutlined />}
            size="small"
            onClick={() => f.value && copy(f.value)}
          />
        </div>
      ))}
    </div>
  );
}
