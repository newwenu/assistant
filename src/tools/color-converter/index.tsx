import { useState, useCallback } from "react";
import { Input, Typography, Button, Slider, App } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import {
  colord,
  FORMATS,
  computeColorValues,
  type ColordInstance,
} from "./color-converter.service";
import styles from "./color-converter.module.css";

export default function ColorConverter() {
  const { message } = App.useApp();
  const [hex, setHex] = useState("#1ea54c");
  const [alpha, setAlpha] = useState(1);
  const [values, setValues] = useState<Record<string, string>>(() => {
    const c = colord("#1ea54c");
    const r: Record<string, string> = {};
    for (const f of FORMATS) {
      r[f.label] = f.format(c);
    }
    r["RGBA"] = c.toRgbString();
    return r;
  });
  const [pickerColor, setPickerColor] = useState("#1ea54c");

  const updateFromColor = useCallback(
    (c: ColordInstance, omitLabel?: string) => {
      if (!c.isValid()) return;
      const newValues = computeColorValues(c, alpha, omitLabel);
      setValues((prev) => ({ ...prev, ...newValues }));
      setHex(c.toHex());
      setPickerColor(c.toHex());
    },
    [alpha],
  );

  const handleHexChange = (value: string) => {
    setHex(value);
    const c = colord(value);
    if (c.isValid()) {
      updateFromColor(c, "HEX");
    }
  };

  const handlePickerChange = (value: string) => {
    setPickerColor(value);
    setHex(value);
    updateFromColor(colord(value), "HEX");
  };

  const handleFormatChange = (label: string, value: string) => {
    setValues((prev) => ({ ...prev, [label]: value }));
    const c = colord(value);
    if (c.isValid()) {
      updateFromColor(c, label);
    }
  };

  const handleAlphaChange = (newAlpha: number) => {
    setAlpha(newAlpha);
    const c = colord(hex);
    if (c.isValid()) {
      const rgb = c.toRgb();
      setValues((prev) => ({
        ...prev,
        RGBA: `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${newAlpha})`,
      }));
    }
  };

  const copy = (value: string) => {
    void navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  const allFormats = [
    ...FORMATS,
    {
      label: "RGBA",
      format: () => values["RGBA"] ?? "",
      placeholder: "rgba(255, 0, 0, 1)",
    },
  ];

  return (
    <div>
      <div className={styles.pickerRow}>
        <input
          type="color"
          value={pickerColor}
          onChange={(e) => handlePickerChange(e.target.value)}
          className={styles.colorPicker}
        />
        <Input
          value={hex}
          onChange={(e) => handleHexChange(e.target.value)}
          placeholder="#1ea54c"
          className={styles.hexInput}
        />
      </div>
      <div className={styles.alphaRow}>
        <Typography.Text strong className={styles.alphaLabel}>
          Alpha
        </Typography.Text>
        <Slider
          min={0}
          max={1}
          step={0.01}
          value={alpha}
          onChange={handleAlphaChange}
          style={{ flex: 1 }}
        />
        <Typography.Text className={styles.alphaValue}>
          {alpha.toFixed(2)}
        </Typography.Text>
      </div>
      {allFormats.map((f) => (
        <div key={f.label} className={styles.formatRow}>
          <Typography.Text strong className={styles.formatLabel}>
            {f.label}
          </Typography.Text>
          <Input
            value={values[f.label] ?? ""}
            onChange={(e) => handleFormatChange(f.label, e.target.value)}
            placeholder={f.placeholder}
            size="small"
            className={styles.formatInput}
          />
          <Button
            icon={<CopyOutlined />}
            size="small"
            onClick={() => values[f.label] && copy(values[f.label])}
          />
        </div>
      ))}
    </div>
  );
}
