import { Form, Slider, Switch, message } from "antd";
import { emit } from "@tauri-apps/api/event";
import { useSettingsStore } from "@/stores/settingsStore";
import type { OrbConf } from "@/stores/settingsStore";

export const OrbAppearanceSection: React.FC = () => {
  const orbConf = useSettingsStore((s) => s.orbConf);
  const saveOrbConf = useSettingsStore((s) => s.saveOrbConf);
  const [messageApi, contextHolder] = message.useMessage();

  const handleSave = async (values: Partial<OrbConf>) => {
    if (!orbConf) return;
    const newConf: OrbConf = { ...orbConf, ...values };
    await saveOrbConf(newConf);
    try {
      await emit("orb-conf-changed", newConf);
    } catch (e) {
      console.error("[OrbAppearanceSection] emit orb-conf-changed failed:", e);
    }
    messageApi.success("已保存");
  };

  return (
    <>
      {contextHolder}
      <Form
        layout="vertical"
        initialValues={{
          opacity_percent: orbConf?.opacity_percent ?? 100,
          scale_percent: orbConf?.scale_percent ?? 100,
          corner_radius_percent: orbConf?.corner_radius_percent ?? 50,
          click_through: orbConf?.click_through ?? false,
          hide_on_pointer_hover: orbConf?.hide_on_pointer_hover ?? false,
          hover_delay_ms: orbConf?.hover_delay_ms ?? 0,
          always_on_top: orbConf?.always_on_top ?? true,
        }}
        onValuesChange={(_changed, all) => handleSave(all)}
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
        <Form.Item
          label="始终置顶"
          name="always_on_top"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
      </Form>
    </>
  );
};