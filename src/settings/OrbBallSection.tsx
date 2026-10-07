import { useEffect } from "react";
import { Form, Slider, Switch, message } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { useDebouncedOrbSave } from "./useDebouncedOrbSave";

export const OrbBallSection: React.FC = () => {
  const orbConf = useSettingsStore((s) => s.orbConf);
  const [messageApi, contextHolder] = message.useMessage();
  const [form] = Form.useForm();
  const handleSave = useDebouncedOrbSave(orbConf, () =>
    messageApi.success("已保存"),
  );

  useEffect(() => {
    if (!orbConf) return;
    form.setFieldsValue({
      opacity_percent: orbConf.opacity_percent,
      scale_percent: orbConf.scale_percent,
      corner_radius_percent: orbConf.corner_radius_percent,
      click_through: orbConf.click_through,
      hide_on_pointer_hover: orbConf.hide_on_pointer_hover,
      hover_delay_ms: orbConf.hover_delay_ms,
    });
  }, [orbConf, form]);

  return (
    <>
      {contextHolder}
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          opacity_percent: orbConf?.opacity_percent ?? 100,
          scale_percent: orbConf?.scale_percent ?? 100,
          corner_radius_percent: orbConf?.corner_radius_percent ?? 50,
          click_through: orbConf?.click_through ?? false,
          hide_on_pointer_hover: orbConf?.hide_on_pointer_hover ?? false,
          hover_delay_ms: orbConf?.hover_delay_ms ?? 0,
        }}
        onValuesChange={(changed, all) => {
          const isSlider = Object.keys(changed).some(
            (k) => k.endsWith("_percent") || k === "hover_delay_ms",
          );
          handleSave(all, isSlider);
        }}
      >
        <Form.Item label="透明度" name="opacity_percent">
          <Slider min={10} max={100} marks={{ 10: "10%", 100: "100%" }} />
        </Form.Item>
        <Form.Item label="缩放" name="scale_percent">
          <Slider
            min={50}
            max={200}
            marks={{ 50: "50%", 100: "100%", 200: "200%" }}
          />
        </Form.Item>
        <Form.Item label="圆角" name="corner_radius_percent">
          <Slider
            min={0}
            max={100}
            marks={{ 0: "0%", 50: "50%", 100: "100%" }}
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
          extra="鼠标悬停在悬浮球上时自动隐藏"
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
    </>
  );
};
