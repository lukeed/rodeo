mod common;

use common::{Harness, run};
use config::Appearance;
use rodeo::app::Rgb;
use rodeo::appearance::COLOR_QUERY;
use rodeo::host_input::Report;

const ASK_BG: &str = r#"stty -icanon -echo min 0 time 10; printf '\033]11;?\033\\'; v=$(dd bs=64 count=1 2>/dev/null); stty sane; printf 'got%s\n' "$v" | tr -d '\033\007'"#;

#[test]
fn host_scheme_reports_switch_the_theme() {
    run(|| async {
        let mut h = Harness::with_config(
            "[theme]\nname = \"tokyo-night\"\nauto_switch = true\n[theme.custom.light]\naccent = \"#123456\"\n",
        )
        .await;
        h.ready().await;
        let config = h.app().config.clone();
        assert_eq!(h.app().palette, config.palette(Some(Appearance::Dark)));
        h.app_mut().host_report(Report::Scheme(Appearance::Light));
        let light = config.palette(Some(Appearance::Light));
        assert_eq!(h.app().palette, light);
        assert_eq!(light.accent, ratatui::style::Color::Rgb(0x12, 0x34, 0x56));
        h.pump().await;
        assert!(
            h.host_output().contains(COLOR_QUERY),
            "a scheme change asks the colors again"
        );
        h.app_mut().host_report(Report::Scheme(Appearance::Dark));
        assert_eq!(h.app().palette, config.palette(Some(Appearance::Dark)));
    });
}

#[test]
fn background_reports_infer_light_without_a_scheme() {
    run(|| async {
        let mut h = Harness::with_config("[theme]\nauto_switch = true\n").await;
        h.ready().await;
        h.app_mut().host_report(Report::Color {
            code: 11,
            rgb: Rgb(0xfa, 0xfa, 0xfa),
        });
        let config = h.app().config.clone();
        assert_eq!(h.app().palette, config.palette(Some(Appearance::Light)));
        assert_eq!(h.app().host.bg, Some(Rgb(0xfa, 0xfa, 0xfa)));
        assert_eq!(h.app().display().bg.as_deref(), Some("#fafafa"));
    });
}

#[test]
fn host_colors_reach_programs_as_display_facts() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.app_mut().host_report(Report::Color {
            code: 11,
            rgb: Rgb(0xfe, 0xdc, 0xba),
        });
        h.pump().await;
        h.typed(&format!("{ASK_BG}\r")).await;
        let address = h.focused_address().expect("a focused pane");
        h.until("the program sees the host background", |h| {
            h.pane_text(&address).contains("got]11;rgb:fefe/dcdc/baba")
        })
        .await;
    });
}
