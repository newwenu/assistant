use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};

pub(crate) type PluginFactory = fn() -> TauriPlugin<Wry>;
pub(crate) type SetupHook = fn(&crate::boot::StartupCtx<'_>) -> Result<(), String>;
pub(crate) type ExitHook = fn(&AppHandle);

pub(crate) struct FeatureMount {
    pub id: &'static str,
    pub plugin: PluginFactory,
    pub on_setup: SetupHook,
    pub on_exit: ExitHook,
}

const MOUNTS: &[FeatureMount] = &[
    FeatureMount {
        id: "vault",
        plugin: crate::vault_plugin::init,
        on_setup: crate::vault_plugin::on_setup,
        on_exit: crate::vault_plugin::on_exit,
    },
    FeatureMount {
        id: "timekeeper",
        plugin: crate::timekeeper_plugin::init,
        on_setup: crate::timekeeper_plugin::on_setup,
        on_exit: crate::timekeeper_plugin::on_exit,
    },
];

pub(crate) fn mounts() -> &'static [FeatureMount] {
    MOUNTS
}
