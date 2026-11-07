use doc::{Content, Document, Kind, Size};
use serde_json::{Value, json};

const SAMPLE: &str = include_str!("fixtures/herdr-app-doc.json");

fn sample() -> Value {
    serde_json::from_str::<Value>(SAMPLE).unwrap()["value"].clone()
}

#[test]
fn live_sample_round_trips() {
    let value = sample();
    let doc = Document::parse(&value).unwrap();
    assert_eq!(doc.spaces.len(), 4);
    assert_eq!(doc.spaces[2].name, "tidy");
    assert_eq!(
        doc.spaces[2].tabs[0].root.terminals()[0].address(),
        Some("t1h")
    );
    assert_eq!(doc.to_value(), value);
}

#[test]
fn live_sample_is_written_in_the_apps_key_order() {
    let doc = Document::parse(&sample()).unwrap();
    let json = doc.to_json();
    assert!(json.starts_with(
        r#"{"spaces":[{"id":"sp874378e9ef2f9d3a940581867df86550","name":"","folder":"/Users/lukee/repos/personal/dotfiles","tabs":[{"id":"tb55e652c308a3d5a693b88db556af86f9","name":"","root":{"id":"tb55e652c308a3d5a693b88db556af86f9","type":"row","children":[{"id":"p296173af484d2549499c25afbafec540","type":"block","content":{"terminal":"t12"}}]}}"#
    ));
    assert!(!json.contains('\n') && !json.contains(": "));
}

const EVERY_FIELD: &str = r##"{"spaces":[{"id":"sp0123456789abcdef0123456789abcdef","name":"work","folder":"/w","tabs":[{"id":"tb1","name":"build","root":{"id":"tb1","cloned_from":{"name":"main","version":"1"},"type":"row","children":[{"id":"s1","type":"column","children":[{"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"weight":0.3333333432674408},"title":"logs"},{"id":"p2","type":"block","content":{"mod":"files","props":{"z":1,"a":[true,null]}},"size":{"weight":0.6666666865348816}}],"size":{"px":260.0},"min":120.0,"max":480.0},{"id":"st","type":"stack","children":[{"id":"c1","type":"block","content":{"tab":"active","pane":{"header":{"content":{"mod":"pane","props":{"k":"v"}},"reveal":"hover","height":28.0,"position":"bottom"},"background":"surface","shadow":"sm","border":{"width":1.0,"color":"border"},"focus":{"width":2.0,"color":"accent"},"radius":10.0,"gap":8.0,"lone":true}}},{"id":"a1","type":"block","content":{"agent":"focused"},"hidden":true}],"active":"c1","header":{"content":"tabs"}},{"id":"m1","type":"block","content":{"mod":"changes"},"size":{"weight":1.0},"hidden":true,"background":{"gradient":["#123a6e","#2a6db3"],"angle":160.0},"border":{"width":1.0,"color":"border"},"radius":6.0,"shadow":"lg","padding":[16.0,12.0],"focus":{"border":{"width":2.0,"color":"accent"},"background":"#00000080"},"theme":{"light":"paper","dark":"ink"},"colors":{"accent":"#ff0000","border":"#333333"},"font":"Berkeley Mono","font_size":13.0,"ui_font":"Inter","terminal_padding":[4.0,2.0],"terminal_opacity":0.8500000238418579,"gap":4.0,"divider":{"width":1.0,"color":"border"},"movable":false,"closable":false,"resizable":false,"floating":[{"id":"f1","content":{"mod":"team"},"edge":"left","align":"start","peek":12.0,"hidden":true},{"id":"f2","content":{"terminal":"t9"},"edge":"right","align":"center"}],"titlebar":"none","traffic_lights":[12.0,18.0],"preview":"layouts/main.svg","terminal_menu":"menu","window_drag":true},{"id":"m2","type":"block","content":{"mod":"x"},"background":{"image":"bg/sky.jpg","fit":"cover"},"padding":8.0,"theme":"ink","titlebar":"system"}]}}]}]}"##;

#[test]
fn every_field_the_app_knows_round_trips_byte_for_byte() {
    let value: Value = serde_json::from_str(EVERY_FIELD).unwrap();
    let doc = Document::parse(&value).unwrap();
    assert_eq!(doc.to_json(), EVERY_FIELD);
    assert_eq!(doc.to_value(), value);
    assert_eq!(Document::parse(&doc.to_value()).unwrap(), doc);
}

#[test]
fn every_node_kind_and_size_form_parses() {
    let doc = Document::parse(&serde_json::from_str(EVERY_FIELD).unwrap()).unwrap();
    let root = &doc.spaces[0].tabs[0].root;
    assert!(matches!(root.kind, Kind::Row { .. }));
    assert!(matches!(root.find("s1").unwrap().kind, Kind::Column { .. }));
    assert!(matches!(root.find("st").unwrap().kind, Kind::Stack { .. }));
    assert_eq!(root.find("s1").unwrap().size, Some(Size::Px { px: 260.0 }));
    assert_eq!(
        root.find("p1").unwrap().size,
        Some(Size::Weight { weight: 1.0 / 3.0 })
    );
    assert!(matches!(
        root.find("c1").unwrap().content(),
        Some(Content::Context { pane: Some(_), .. })
    ));
    assert!(matches!(
        root.find("a1").unwrap().content(),
        Some(Content::Agent { .. })
    ));
    assert!(matches!(
        root.find("p2").unwrap().content(),
        Some(Content::Mod { .. })
    ));
    assert_eq!(
        root.panes()
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        ["p1", "p2", "m1", "m2"]
    );
}

#[test]
fn app_defaults_fill_missing_fields_and_are_written_out() {
    let doc = Document::parse(&json!({"spaces":[{"tabs":[{}]}]})).unwrap();
    assert_eq!(
        doc.to_value(),
        json!({"spaces":[{"id":"","name":"","folder":"","tabs":[{"id":"","name":"","root":{"id":"","type":"row","children":[]}}]}]})
    );
    let doc = Document::parse(&json!({"spaces":[{"id":"sp","tabs":[{"id":"t","root":{"type":"row","children":[
        {"type":"block","content":{"tab":"x"}},
        {"type":"block","content":{"context":"y"}},
        {"type":"block","content":{"mod":"m","props":null}},
        {"type":"block","content":{"mod":"n"},"border":{},"background":{"gradient":["#000","#fff"]}},
        {"type":"block","content":{"mod":"o"},"background":{"image":"i.png"},"floating":[{"id":"f","content":{"agent":"a"}}]}
    ]}}]}]}))
    .unwrap();
    assert_eq!(
        doc.to_value()["spaces"][0]["tabs"][0]["root"]["children"],
        json!([
            {"id":"","type":"block","content":{"tab":"x"}},
            {"id":"","type":"block","content":{"tab":"y"}},
            {"id":"","type":"block","content":{"mod":"m"}},
            {"id":"","type":"block","content":{"mod":"n"},"background":{"gradient":["#000","#fff"],"angle":180.0},"border":{"width":1.0,"color":"border"}},
            {"id":"","type":"block","content":{"mod":"o"},"background":{"image":"i.png","fit":"cover"},"floating":[{"id":"f","content":{"agent":"a"},"edge":"right","align":"center"}]}
        ])
    );
}

#[test]
fn null_and_empty_values_are_an_empty_doc() {
    assert_eq!(Document::parse(&Value::Null).unwrap(), Document::default());
    assert_eq!(Document::parse(&json!({})).unwrap(), Document::default());
    assert_eq!(Document::default().to_json(), r#"{"spaces":[]}"#);
}

#[test]
fn unknown_fields_are_dropped_like_the_app_drops_them() {
    let doc = Document::parse(&json!({"extra":1,"spaces":[{"id":"sp","x":2,"tabs":[{"id":"t","y":3,"root":{"id":"t","type":"row","z":4,"children":[]}}]}]})).unwrap();
    assert_eq!(
        doc.to_value(),
        json!({"spaces":[{"id":"sp","name":"","folder":"","tabs":[{"id":"t","name":"","root":{"id":"t","type":"row","children":[]}}]}]})
    );
}

#[test]
fn what_the_app_cannot_parse_is_rejected() {
    let bad = [
        json!({"spaces":[{"tabs":[{"root":{"type":"grid","children":[]}}]}]}),
        json!({"spaces":[{"tabs":[{"root":{"children":[]}}]}]}),
        json!({"spaces":[{"tabs":[{"root":{"type":"row"}}]}]}),
        json!({"spaces":[{"tabs":[{"root":{"type":"block"}}]}]}),
        json!({"spaces":[{"tabs":[{"root":{"type":"block","content":{"browser":"x"}}}]}]}),
        json!({"spaces":[{"tabs":[{"root":{"type":"row","children":[],"size":{"weight":null}}}]}]}),
        json!({"spaces":[{"tabs":[{"root":{"type":"row","children":[],"titlebar":"custom"}}]}]}),
        json!({"spaces":"none"}),
    ];
    for value in bad {
        assert!(Document::parse(&value).is_err(), "{value}");
    }
}

#[test]
fn hand_written_numbers_take_the_apps_f32_form() {
    let doc = Document::parse(&json!({"spaces":[{"tabs":[{"root":{"type":"row","children":[],"size":{"weight":0.7},"font_size":13}}]}]})).unwrap();
    assert_eq!(
        doc.to_value()["spaces"][0]["tabs"][0]["root"],
        json!({"id":"","type":"row","children":[],"size":{"weight":0.699999988079071},"font_size":13.0})
    );
}
