import { useState, useMemo } from "react";
import { Select, InputNumber, Typography } from "antd";
import {
  CATEGORIES,
  CATEGORY_OPTIONS,
  computeConversions,
} from "./unit-converter.service";
import styles from "./unit-converter.module.css";

export default function UnitConverter() {
  const [category, setCategory] = useState("length");
  const [fromUnit, setFromUnit] = useState("m");
  const [value, setValue] = useState<number | null>(1);

  const unitOptions = useMemo(() => {
    const cat = CATEGORIES[category];
    if (!cat) return [];
    return Object.entries(cat.units).map(([key, u]) => ({
      label: u.name,
      value: key,
    }));
  }, [category]);

  const conversions = useMemo(() => {
    if (value === null || value === undefined) return [];
    return computeConversions(category, fromUnit, value);
  }, [category, fromUnit, value]);

  const handleCategoryChange = (cat: string) => {
    setCategory(cat);
    const firstUnit = Object.keys(CATEGORIES[cat]?.units ?? {})[0];
    setFromUnit(firstUnit);
  };

  return (
    <div>
      <div className={styles.configRow}>
        <div className={styles.configItem}>
          <Typography.Text strong className={styles.configLabel}>
            类别
          </Typography.Text>
          <Select
            value={category}
            onChange={handleCategoryChange}
            options={CATEGORY_OPTIONS}
            style={{ flex: 1 }}
          />
        </div>
        <div className={styles.configItem}>
          <Typography.Text strong className={styles.configLabel}>
            源单位
          </Typography.Text>
          <Select
            value={fromUnit}
            onChange={setFromUnit}
            options={unitOptions}
            style={{ flex: 1 }}
          />
        </div>
        <div className={styles.configItem}>
          <Typography.Text strong className={styles.configLabel}>
            数值
          </Typography.Text>
          <InputNumber
            value={value}
            onChange={(v) => setValue(v)}
            style={{ flex: 1 }}
            min={-1e15}
            max={1e15}
          />
        </div>
      </div>

      <div className={styles.results}>
        {conversions.map((c) => (
          <div key={c.key} className={styles.resultRow}>
            <Typography.Text strong className={styles.resultName}>
              {c.name}
            </Typography.Text>
            <Typography.Text className={styles.resultValue}>
              {c.value.toPrecision(10).replace(/\.?0+$/, "")}
            </Typography.Text>
          </div>
        ))}
      </div>
    </div>
  );
}
