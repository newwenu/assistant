import { useEffect } from "react";
import { Form, Slider, Switch } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { useDebouncedOrbSave } from "./useDebouncedOrbSave";

interface OrbBallSectionProps {
  onSaved?: () => void;
}

export const OrbBallSection: React.FC<OrbBallSectionProps> = ({ onSaved }) => {
  const orbConf = useSettingsStore((s) => s.orbConf);
  const [form] = Form.useForm();
  const handleSave = useDebouncedOrbSave(orbConf, onSaved ?? (() => {}));

  useEffect(() => {
    if (!orbConf) return;
    form.setFieldsValue({
      opacity_percent: orbConf.opacity_percent,
      scale_percent: orbConf.scale_percent,
      click_through: orbConf.click_through,
      hide_on_pointer_hover: orbConf.hide_on_pointer_hover,
      hover_delay_ms: orbConf.hover_delay_ms,
      idle_opacity_percent: orbConf.idle_opacity_percent,
    });
  }, [orbConf, form]);

  return (
    <Form
      form={form}
      layout="vertical"
      initialValues={{
        opacity_percent: orbConf?.opacity_percent ?? 100,
        scale_percent: orbConf?.scale_percent ?? 100,
        click_through: orbConf?.click_through ?? false,
        hide_on_pointer_hover: orbConf?.hide_on_pointer_hover ?? false,
        hover_delay_ms: orbConf?.hover_delay_ms ?? 0,
        idle_opacity_percent: orbConf?.idle_opacity_percent ?? 85,
      }}
      onValuesChange={(changed, all) => {
        const isSlider = Object.keys(changed).some(
          (k) => k.endsWith("_percent") || k === "hover_delay_ms",
        );
        handleSave(all, isSlider);
      }}
    >
      <Form.Item label="悬停透明度" name="opacity_percent">
        <Slider min={10} max={100} marks={{ 10: "10%", 100: "100%" }} />
      </Form.Item>
      <Form.Item label="空闲透明度" name="idle_opacity_percent">
        <Slider
          min={10}
          max={100}
          marks={{ 10: "10%", 85: "85%", 100: "100%" }}
        />
      </Form.Item>
      <Form.Item label="缩放" name="scale_percent">
        <Slider
          min={50}
          max={200}
          marks={{ 50: "50%", 100: "100%", 200: "200%" }}
        />
      </Form.Item>
      <Form.Item
        label="鼠标穿透"
        name="click_through"
        valuePropName="checked"
        extra="开启后鼠标可穿透悬浮球点击下方内容"
      >
        <Switch />
      </Form.Item>
      <Form.Item
        label="悬停隐藏"
        name="hide_on_pointer_hover"
        valuePropName="checked"
      >
        <Switch />
      </Form.Item>
      <Form.Item label="悬停延迟 (ms)" name="hover_delay_ms">
        <Slider
          min={0}
          max={2000}
          step={100}
          marks={{ 0: "0", 1000: "1s", 2000: "2s" }}
        />
      </Form.Item>
    </Form>
  );
};
