#![cfg(feature = "slow-tests")]

use assistant_config::app_conf::AppConf;
use proptest::prelude::*;

proptest! {
    #[test]
    fn validate_theme_only_system_light_dark(theme in "[a-zA-Z]{1,10}") {
        let mut conf = AppConf::new();
        conf.theme = theme.clone();
        let valid = ["system", "light", "dark"];
        assert_eq!(conf.validate().is_ok(), valid.contains(&theme.as_str()));
    }

    #[test]
    fn serde_roundtrip_preserves_validate(theme in "(system|light|dark)", language in "[a-zA-Z-]{1,10}") {
        let mut conf = AppConf::new();
        conf.theme = theme;
        conf.language = language;
        let json = serde_json::to_string(&conf).unwrap();
        let decoded: AppConf = serde_json::from_str(&json).unwrap();
        assert_eq!(conf.validate().is_ok(), decoded.validate().is_ok());
    }
}
