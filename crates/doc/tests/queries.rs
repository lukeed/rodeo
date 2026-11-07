use std::collections::HashSet;

use doc::{Document, Node, Prefix, Space, Tab, folder_name, fresh_id, is_id, where_label};
use serde_json::json;

fn sample() -> Document {
    Document::parse(&json!({"spaces": [
        {"id":"sp1","name":"","folder":"/home/me/src/api","tabs":[
            {"id":"tb1","name":"","root":{"id":"tb1","type":"row","children":[
                {"id":"p1","type":"block","content":{"terminal":"t1"}},
                {"id":"s1","type":"column","children":[
                    {"id":"p2","type":"block","content":{"mod":"files"}},
                    {"id":"p3","type":"block","content":{"terminal":"t3"}},
                    {"id":"p4","type":"block","content":{"terminal":"t4"},"hidden":true},
                ]},
                {"id":"x1","type":"block","content":{"agent":"focused"}},
            ]}},
            {"id":"tb2","name":"logs","root":{"id":"tb2","type":"row","children":[
                {"id":"p5","type":"block","content":{"terminal":"t5"}},
            ]}},
        ]},
        {"id":"sp2","name":"tidy","folder":"/x","tabs":[
            {"id":"tb3","name":"","root":{"id":"tb3","type":"row","children":[
                {"id":"p6","type":"block","content":{"mod":"changes"}},
            ]}},
        ]},
    ]}))
    .unwrap()
}

fn ids(nodes: &[&Node]) -> Vec<String> {
    nodes.iter().map(|n| n.id.clone()).collect()
}

#[test]
fn fresh_ids_look_like_the_apps() {
    for (prefix, text) in [
        (Prefix::Space, "sp"),
        (Prefix::Tab, "tb"),
        (Prefix::Pane, "p"),
        (Prefix::Split, "s"),
    ] {
        let id = fresh_id(prefix);
        assert_eq!(id.len(), text.len() + 32, "{id}");
        assert!(id.starts_with(text));
        assert!(is_id(&id, prefix));
    }
    let many: HashSet<String> = (0..1000).map(|_| fresh_id(Prefix::Pane)).collect();
    assert_eq!(many.len(), 1000);
    assert!(!is_id("p0123456789ABCDEF0123456789abcdef", Prefix::Pane));
    assert!(!is_id("p0123456789abcdef", Prefix::Pane));
    assert!(!is_id("tb0123456789abcdef0123456789abcdef", Prefix::Pane));
    assert!(is_id("tb0123456789abcdef0123456789abcdef", Prefix::Tab));
}

#[test]
fn doc_ids_are_unique_across_the_doc() {
    let d = sample();
    for id in ["sp1", "tb2", "s1", "p6", "x1"] {
        assert!(d.contains_id(id), "{id}");
    }
    assert!(!d.contains_id("p9"));
    let id = d.fresh_id(Prefix::Tab);
    assert!(is_id(&id, Prefix::Tab) && !d.contains_id(&id));
}

#[test]
fn finds_spaces_tabs_and_panes() {
    let d = sample();
    assert_eq!(d.space("sp2").unwrap().name, "tidy");
    assert_eq!(d.tab("tb2").unwrap().name, "logs");
    assert_eq!(d.space_of("tb3").unwrap().id, "sp2");
    assert_eq!(d.space_of("p3").unwrap().id, "sp1");
    assert_eq!(d.tab_of("p3").unwrap().id, "tb1");
    assert_eq!(d.tab_index("tb2"), Some(1));
    assert_eq!(d.tab_index("tb3"), Some(0));
    assert_eq!(d.tab_index("p1"), None);
    let found = d.pane("p2").unwrap();
    assert_eq!(
        (found.space.id.as_str(), found.tab.id.as_str()),
        ("sp1", "tb1")
    );
    assert!(d.pane("s1").is_none());
    assert!(d.pane("x1").is_none());
    assert_eq!(d.locate("s1").unwrap().tab.id, "tb1");
    let found = d.pane_of_terminal("t5").unwrap();
    assert_eq!(
        (found.tab.id.as_str(), found.node.id.as_str()),
        ("tb2", "p5")
    );
    assert!(d.pane_of_terminal("t9").is_none());
}

#[test]
fn lists_terminals_and_panes_in_tree_order() {
    let d = sample();
    let tab = d.tab("tb1").unwrap();
    assert_eq!(ids(&tab.panes()), ["p1", "p2", "p3", "p4"]);
    assert_eq!(ids(&tab.terminals()), ["p1", "p3", "p4"]);
    assert_eq!(tab.addresses(), ["t1", "t3", "t4"]);
    assert_eq!(ids(&tab.root.visible_panes()), ["p1", "p2", "p3"]);
    let space = d.space("sp1").unwrap();
    assert_eq!(ids(&space.terminals()), ["p1", "p3", "p4", "p5"]);
    assert_eq!(space.addresses(), ["t1", "t3", "t4", "t5"]);
    assert_eq!(space.first_terminal().unwrap().id, "p1");
    assert!(d.space("sp2").unwrap().first_terminal().is_none());
    assert_eq!(d.addresses(), ["t1", "t3", "t4", "t5"]);
}

#[test]
fn pane_cycle_wraps_over_visible_panes() {
    let d = sample();
    let tab = d.tab("tb1").unwrap();
    assert_eq!(tab.cycle("p1", true), Some("p2"));
    assert_eq!(tab.cycle("p3", true), Some("p1"));
    assert_eq!(tab.cycle("p1", false), Some("p3"));
    assert_eq!(tab.cycle("p4", true), None);
    let lone = d.tab("tb2").unwrap();
    assert_eq!(lone.cycle("p5", true), Some("p5"));
}

#[test]
fn stacks_show_their_active_child() {
    let d = Document::parse(
        &json!({"spaces":[{"tabs":[{"id":"t","root":{"id":"t","type":"row","children":[
            {"id":"st","type":"stack","active":"b","children":[
                {"id":"a","type":"block","content":{"terminal":"a"}},
                {"id":"b","type":"block","content":{"terminal":"b"}},
            ]},
            {"id":"st2","type":"stack","active":"d","children":[
                {"id":"c","type":"block","content":{"terminal":"c"}},
                {"id":"d","type":"block","content":{"terminal":"d"},"hidden":true},
            ]},
        ]}}]}]}),
    )
    .unwrap();
    let root = &d.tab("t").unwrap().root;
    assert_eq!(ids(&root.visible_panes()), ["b", "c"]);
    assert_eq!(root.find("st2").unwrap().active_child().unwrap().id, "c");
}

#[test]
fn folder_names_follow_the_app() {
    assert_eq!(folder_name("/home/can", "/home/can"), "~");
    assert_eq!(folder_name("/home/can/", "/home/can"), "~");
    assert_eq!(
        folder_name("/Users/can/Projects/herdr", "/Users/can"),
        "herdr"
    );
    assert_eq!(folder_name("/", "/home/can"), "/");
    assert_eq!(folder_name("", "/home/can"), "~");
    assert_eq!(folder_name(r"C:\Users\can\src\api", r"C:\Users\can"), "api");
    assert_eq!(folder_name("C:/Users/can/src/web", r"C:\Users\can"), "web");
}

#[test]
fn space_and_tab_names_and_the_where_label() {
    let d = sample();
    let api = d.space("sp1").unwrap();
    assert_eq!(api.display_name(None, "/home/me"), "api");
    assert_eq!(
        api.display_name(Some("/home/me/src/web"), "/home/me"),
        "web"
    );
    assert_eq!(api.display_name(Some("/home/me"), "/home/me"), "~");
    assert_eq!(
        d.space("sp2")
            .unwrap()
            .display_name(Some("/elsewhere"), "/home/me"),
        "tidy"
    );
    assert_eq!(api.tab_name("tb1").as_deref(), Some("1"));
    assert_eq!(api.tab_name("tb2").as_deref(), Some("logs"));
    assert_eq!(api.tab_name("tb3"), None);
    assert_eq!(where_label("api", "2"), "api / 2");
    assert_eq!(
        d.where_label("tb1", None, "/home/me").as_deref(),
        Some("api / 1")
    );
    assert_eq!(
        d.where_label("tb3", None, "/home/me").as_deref(),
        Some("tidy / 1")
    );
    assert_eq!(d.where_label("p1", None, "/home/me"), None);
    let blank = Space {
        id: "s".into(),
        folder: String::new(),
        ..Space::default()
    };
    assert_eq!(blank.display_name(None, "/home/me"), "~");
}

#[test]
fn tab_new_puts_the_pane_in_a_root_row_named_after_the_tab() {
    let tab = Tab::new("tb1", "x", Node::terminal("p1", "t1"));
    assert_eq!(
        serde_json::to_value(&tab).unwrap(),
        json!({"id":"tb1","name":"x","root":{"id":"tb1","type":"row","children":[{"id":"p1","type":"block","content":{"terminal":"t1"}}]}})
    );
}
