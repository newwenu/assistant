#![cfg(feature = "slow-tests")]

use assistant_config::orb_conf::OrbConf;
use proptest::prelude::*;

proptest! {
    #[test]
    fn validate_scale_bounds(scale in 0u16..400) {
        let mut conf = OrbConf::new();
        conf.scale_percent = scale;
        assert_eq!(conf.validate().is_ok(), (10..=300).contains(&scale));
    }

    #[test]
    fn validate_opacity_bounds(opacity in 0u8..120) {
        let mut conf = OrbConf::new();
        conf.opacity_percent = opacity;
        assert_eq!(conf.validate().is_ok(), (10..=100).contains(&opacity));
    }

    #[test]
    fn validate_hover_delay_bounds(delay in 0u64..6000) {
        let mut conf = OrbConf::new();
        conf.hover_delay_ms = delay;
        assert_eq!(conf.validate().is_ok(), delay <= 5000);
    }

    #[test]
    fn validate_idle_opacity_bounds(idle_opacity in 0u8..120) {
        let mut conf = OrbConf::new();
        conf.idle_opacity_percent = idle_opacity;
        assert_eq!(conf.validate().is_ok(), (10..=100).contains(&idle_opacity));
    }

    #[test]
    fn serde_roundtrip_preserves_validate(
        scale in 10u16..300,
        opacity in 10u8..100,
        delay in 0u64..5000,
        idle_opacity in 10u8..100,
    ) {
        let mut conf = OrbConf::new();
        conf.scale_percent = scale;
        conf.opacity_percent = opacity;
        conf.hover_delay_ms = delay;
        conf.idle_opacity_percent = idle_opacity;
        let json = serde_json::to_string(&conf).unwrap();
        let decoded: OrbConf = serde_json::from_str(&json).unwrap();
        assert!(decoded.validate().is_ok());
        assert_eq!(conf.validate().is_ok(), decoded.validate().is_ok());
    }
}
