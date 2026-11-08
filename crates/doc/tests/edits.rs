use doc::{Adopted, Axis, Closed, Document, Error, Node, Place, Prefix, Tab, is_id};
use serde_json::{Value, json};

fn doc(value: Value) -> Document {
    serde_json::from_value(value).unwrap()
}

fn term(id: &str, address: &str) -> Value {
    json!({"id": id, "type": "block", "content": {"terminal": address}})
}

fn space(id: &str, tabs: Value) -> Value {
    let mut space = json!({"id": id, "name": "", "folder": "/w"});
    space["tabs"] = tabs;
    space
}

fn tab(id: &str, children: Value) -> Value {
    let mut tab = json!({"id": id, "name": "", "root": {"id": id, "type": "row"}});
    tab["root"]["children"] = children;
    tab
}

fn root(d: &Document, tab: &str) -> Value {
    serde_json::to_value(&d.tab(tab).unwrap().root).unwrap()
}

fn two_spaces() -> Document {
    doc(json!({"spaces": [
        space("sp1", json!([tab("tb1", json!([term("p1", "t1"), term("p2", "t2")])), tab("tb2", json!([term("p3", "t3")]))])),
        space("sp2", json!([tab("tb3", json!([term("p4", "t4")]))])),
    ]}))
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn new_space_is_added_last_with_trimmed_names() {
    let mut d = two_spaces();
    d.new_space(
        "sp9",
        "  work ",
        "/src",
        Tab::new("tb9", " build ", Node::terminal("p9", "t9")),
    )
    .unwrap();
    assert_eq!(
        d.to_value()["spaces"][2],
        json!({"id":"sp9","name":"work","folder":"/src","tabs":[{"id":"tb9","name":"build","root":{"id":"tb9","type":"row","children":[{"id":"p9","type":"block","content":{"terminal":"t9"}}]}}]})
    );
    let mut d = Document::default();
    d.new_space(
        "sp1",
        "",
        "",
        Tab::new("tb1", "", Node::module("p1", "files", Value::Null)),
    )
    .unwrap();
    assert_eq!(
        d.to_json(),
        r#"{"spaces":[{"id":"sp1","name":"","folder":"","tabs":[{"id":"tb1","name":"","root":{"id":"tb1","type":"row","children":[{"id":"p1","type":"block","content":{"mod":"files"}}]}}]}]}"#
    );
}

#[test]
fn new_ids_must_be_free() {
    let mut d = two_spaces();
    let tab = |t: &str, p: &str| Tab::new(t, "", Node::terminal(p, "t9"));
    assert_eq!(
        d.new_space("sp1", "", "", tab("tb9", "p9")),
        Err(Error::IdTaken("sp1".into()))
    );
    assert_eq!(
        d.new_space("sp9", "", "", tab("tb1", "p9")),
        Err(Error::IdTaken("tb1".into()))
    );
    assert_eq!(
        d.new_tab("sp1", tab("tb9", "p3"), None),
        Err(Error::IdTaken("p3".into()))
    );
    assert_eq!(
        d.split_pane("p1", Axis::Row, Node::terminal("tb2", "t9")),
        Err(Error::IdTaken("tb2".into()))
    );
    assert_eq!(d, two_spaces());
}

#[test]
fn close_space_reports_every_terminal() {
    let mut d = two_spaces();
    let closed = d.close_space("sp1").unwrap();
    assert_eq!(
        closed,
        Closed {
            panes: strings(&["p1", "p2", "p3"]),
            tabs: strings(&["tb1", "tb2"]),
            space: Some("sp1".into()),
            terminals: strings(&["t1", "t2", "t3"]),
        }
    );
    assert_eq!(
        d.to_value(),
        json!({"spaces": [space("sp2", json!([tab("tb3", json!([term("p4", "t4")]))]))]})
    );
    assert_eq!(d.close_space("sp1"), Err(Error::NoSpace("sp1".into())));
}

#[test]
fn new_tab_goes_after_the_given_tab_else_last() {
    let mut d = two_spaces();
    d.new_tab(
        "sp1",
        Tab::new("tb8", " x ", Node::terminal("p8", "t8")),
        Some("tb1"),
    )
    .unwrap();
    d.new_tab("sp1", Tab::new("tb9", "", Node::terminal("p9", "t9")), None)
        .unwrap();
    d.new_tab(
        "sp1",
        Tab::new("tb7", "", Node::terminal("p7", "t7")),
        Some("tb3"),
    )
    .unwrap();
    let ids: Vec<_> = d
        .space("sp1")
        .unwrap()
        .tabs
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(ids, ["tb1", "tb8", "tb2", "tb9", "tb7"]);
    assert_eq!(d.tab("tb8").unwrap().name, "x");
    assert_eq!(
        d.new_tab(
            "nope",
            Tab::new("tb6", "", Node::terminal("p6", "t6")),
            None
        ),
        Err(Error::NoSpace("nope".into()))
    );
}

#[test]
fn close_tab_takes_its_space_when_it_is_the_last() {
    let mut d = two_spaces();
    let closed = d.close_tab("tb1").unwrap();
    assert_eq!(
        closed,
        Closed {
            panes: strings(&["p1", "p2"]),
            tabs: strings(&["tb1"]),
            space: None,
            terminals: strings(&["t1", "t2"]),
        }
    );
    assert_eq!(d.space("sp1").unwrap().tabs.len(), 1);
    let closed = d.close_tab("tb2").unwrap();
    assert_eq!(closed.space.as_deref(), Some("sp1"));
    assert_eq!(closed.terminals, strings(&["t3"]));
    assert!(d.space("sp1").is_none());
    assert_eq!(d.close_tab("tb2"), Err(Error::NoTab("tb2".into())));
}

#[test]
fn renames_are_trimmed_and_empty_means_automatic() {
    let mut d = two_spaces();
    d.rename_space("sp1", "  api  ").unwrap();
    d.rename_tab("tb2", "\tlogs\n").unwrap();
    assert_eq!(d.space("sp1").unwrap().name, "api");
    assert_eq!(d.tab("tb2").unwrap().name, "logs");
    d.rename_space("sp1", "   ").unwrap();
    d.rename_tab("tb2", "").unwrap();
    assert_eq!(d, two_spaces());
    assert_eq!(d.rename_tab("x", "y"), Err(Error::NoTab("x".into())));
    assert_eq!(d.rename_space("x", "y"), Err(Error::NoSpace("x".into())));
}

fn three_tabs() -> Document {
    doc(json!({"spaces": [space("sp1", json!([
        tab("a", json!([term("pa", "ta")])),
        tab("b", json!([term("pb", "tb")])),
        tab("c", json!([term("pc", "tc")])),
    ]))]}))
}

fn order(d: &Document) -> Vec<&str> {
    d.spaces[0].tabs.iter().map(|t| t.id.as_str()).collect()
}

#[test]
fn tabs_move_to_an_index() {
    let mut d = three_tabs();
    d.move_tab("a", 2).unwrap();
    assert_eq!(order(&d), ["b", "c", "a"]);
    d.move_tab("a", 0).unwrap();
    assert_eq!(order(&d), ["a", "b", "c"]);
    d.move_tab("b", 99).unwrap();
    assert_eq!(order(&d), ["a", "c", "b"]);
    assert_eq!(d.move_tab("z", 0), Err(Error::NoTab("z".into())));
}

fn five_spaces() -> Document {
    doc(json!({"spaces": [
        space("a", json!([tab("ta", json!([term("pa", "t1")]))])),
        space("b", json!([tab("tb", json!([term("pb", "t2")]))])),
        space("c", json!([tab("tc", json!([term("pc", "t3")]))])),
        space("d", json!([tab("td", json!([term("pd", "t4")]))])),
        space("e", json!([tab("te", json!([term("pe", "t5")]))])),
    ]}))
}

fn space_order(d: &Document) -> Vec<&str> {
    d.spaces.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn spaces_move_as_a_block_before_another() {
    let ids = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let mut d = five_spaces();
    d.move_spaces(&ids(&["c", "a", "d"]), Some("e")).unwrap();
    assert_eq!(space_order(&d), ["b", "c", "a", "d", "e"]);
    d.move_spaces(&ids(&["e"]), Some("b")).unwrap();
    assert_eq!(space_order(&d), ["e", "b", "c", "a", "d"]);
    d.move_spaces(&ids(&["e"]), None).unwrap();
    assert_eq!(space_order(&d), ["b", "c", "a", "d", "e"]);
    assert_eq!(
        d.move_spaces(&ids(&["z"]), None),
        Err(Error::NoSpace("z".into()))
    );
    assert_eq!(
        d.move_spaces(&ids(&["a", "a"]), None),
        Err(Error::NoSpace("a".into()))
    );
    assert_eq!(
        d.move_spaces(&ids(&["a"]), Some("a")),
        Err(Error::NoSpace("a".into()))
    );
    assert_eq!(space_order(&d), ["b", "c", "a", "d", "e"]);
}

#[test]
fn tabs_step_with_wrap() {
    let mut d = three_tabs();
    d.step_tab("a", true).unwrap();
    assert_eq!(order(&d), ["b", "a", "c"]);
    d.step_tab("c", true).unwrap();
    assert_eq!(order(&d), ["c", "b", "a"]);
    d.step_tab("c", false).unwrap();
    assert_eq!(order(&d), ["b", "a", "c"]);
    d.step_tab("a", false).unwrap();
    assert_eq!(order(&d), ["a", "b", "c"]);
    let mut lone = two_spaces();
    lone.step_tab("tb3", true).unwrap();
    assert_eq!(lone, two_spaces());
}

#[test]
fn split_on_the_same_axis_inserts_and_halves_the_weight() {
    let mut d = two_spaces();
    let container = d
        .split_pane("p1", Axis::Row, Node::terminal("p9", "t9"))
        .unwrap();
    assert_eq!(container, None);
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([
            {"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"weight":0.5}},
            {"id":"p9","type":"block","content":{"terminal":"t9"},"size":{"weight":0.5}},
            term("p2", "t2"),
        ])
    );
    d.split_pane("p9", Axis::Row, Node::terminal("p8", "t8"))
        .unwrap();
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([
            {"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"weight":0.5}},
            {"id":"p9","type":"block","content":{"terminal":"t9"},"size":{"weight":0.25}},
            {"id":"p8","type":"block","content":{"terminal":"t8"},"size":{"weight":0.25}},
            term("p2", "t2"),
        ])
    );
}

#[test]
fn split_across_the_axis_wraps_the_target_in_a_new_container() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        {"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"weight":0.5},"title":"keep"},
        term("p2", "t2"),
    ]))]))]}));
    let container = d
        .split_pane("p1", Axis::Column, Node::terminal("p9", "t9"))
        .unwrap()
        .unwrap();
    assert!(is_id(&container, Prefix::Split));
    assert_eq!(
        root(&d, "tb1"),
        json!({"id":"tb1","type":"row","children":[
            {"id": container, "type":"column","children":[
                {"id":"p1","type":"block","content":{"terminal":"t1"},"title":"keep"},
                term("p9", "t9"),
            ],"size":{"weight":0.5}},
            term("p2", "t2"),
        ]})
    );
    d.split_pane("p9", Axis::Column, Node::terminal("p8", "t8"))
        .unwrap();
    assert_eq!(
        root(&d, "tb1")["children"][0]["children"],
        json!([
            {"id":"p1","type":"block","content":{"terminal":"t1"},"title":"keep"},
            {"id":"p9","type":"block","content":{"terminal":"t9"},"size":{"weight":0.5}},
            {"id":"p8","type":"block","content":{"terminal":"t8"},"size":{"weight":0.5}},
        ])
    );
}

#[test]
fn split_keeps_a_px_target_and_gives_the_new_pane_half_of_one() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        {"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"px":300.0}},
    ]))]))]}));
    d.split_pane(
        "p1",
        Axis::Row,
        Node::module("p9", "files", json!({"path": "/"})),
    )
    .unwrap();
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([
            {"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"px":300.0}},
            {"id":"p9","type":"block","content":{"mod":"files","props":{"path":"/"}},"size":{"weight":0.5}},
        ])
    );
}

#[test]
fn split_needs_a_pane() {
    let mut d = two_spaces();
    assert_eq!(
        d.split_pane("tb1", Axis::Row, Node::terminal("p9", "t9")),
        Err(Error::NoPane("tb1".into()))
    );
    assert_eq!(
        d.split_pane("zz", Axis::Row, Node::terminal("p9", "t9")),
        Err(Error::NoPane("zz".into()))
    );
}

#[test]
fn closing_a_pane_hands_its_weight_to_the_previous_sibling_else_the_next() {
    let weighted = |id: &str, w: f32| json!({"id": id, "type":"block","content":{"terminal": id},"size":{"weight": w}});
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        weighted("a", 0.5), weighted("b", 0.25), weighted("c", 0.25),
    ]))]))]}));
    let closed = d.close_pane("b").unwrap();
    assert_eq!(
        closed,
        Closed {
            panes: strings(&["b"]),
            terminals: strings(&["b"]),
            ..Closed::default()
        }
    );
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([weighted("a", 0.75), weighted("c", 0.25)])
    );
    d.close_pane("a").unwrap();
    assert_eq!(root(&d, "tb1")["children"], json!([weighted("c", 1.0)]));
}

#[test]
fn closing_a_pane_without_size_makes_its_heir_weight_explicit() {
    let mut d = two_spaces();
    d.close_pane("p2").unwrap();
    assert_eq!(
        root(&d, "tb1"),
        json!({"id":"tb1","type":"row","children":[{"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"weight":2.0}}]})
    );
}

#[test]
fn px_sizes_are_not_handed_over() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        {"id":"a","type":"block","content":{"terminal":"a"},"size":{"px":200.0}},
        term("b", "b"),
        term("c", "c"),
    ]))]))]}));
    d.close_pane("b").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([{"id":"a","type":"block","content":{"terminal":"a"},"size":{"px":200.0}}, term("c", "c")])
    );
    d.close_pane("a").unwrap();
    assert_eq!(root(&d, "tb1")["children"], json!([term("c", "c")]));
}

#[test]
fn a_container_left_with_one_child_gives_way_to_it() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        term("a", "a"),
        {"id":"s1","type":"column","size":{"weight":0.5},"children":[
            {"id":"b","type":"block","content":{"terminal":"b"},"size":{"weight":3.0}},
            term("c", "c"),
        ]},
    ]))]))]}));
    d.close_pane("c").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([term("a", "a"), {"id":"b","type":"block","content":{"terminal":"b"},"size":{"weight":0.5}}])
    );
}

#[test]
fn removal_collapses_the_whole_tab_like_the_app() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        term("a", "a"),
        term("b", "b"),
        {"id":"s1","type":"column","children":[term("c", "c")]},
        {"id":"s2","type":"row","children":[]},
    ]))]))]}));
    d.close_pane("b").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([{"id":"a","type":"block","content":{"terminal":"a"},"size":{"weight":2.0}}, term("c", "c")])
    );
}

#[test]
fn stacks_are_kept_and_pick_the_next_active_child() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        term("a", "a"),
        {"id":"st","type":"stack","active":"b","children":[term("b", "b"), term("c", "c")]},
    ]))]))]}));
    d.close_pane("b").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"][1],
        json!({"id":"st","type":"stack","children":[term("c", "c")],"active":"c"})
    );
    d.close_pane("c").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"][1],
        json!({"id":"st","type":"stack","children":[]})
    );
}

#[test]
fn the_last_pane_closes_its_tab_and_the_last_tab_its_space() {
    let mut d = two_spaces();
    let closed = d.close_pane("p3").unwrap();
    assert_eq!(
        closed,
        Closed {
            panes: strings(&["p3"]),
            tabs: strings(&["tb2"]),
            space: None,
            terminals: strings(&["t3"])
        }
    );
    let closed = d.close_pane("p4").unwrap();
    assert_eq!(
        closed,
        Closed {
            panes: strings(&["p4"]),
            tabs: strings(&["tb3"]),
            space: Some("sp2".into()),
            terminals: strings(&["t4"])
        }
    );
    assert_eq!(
        d.to_value(),
        json!({"spaces": [space("sp1", json!([tab("tb1", json!([term("p1", "t1"), term("p2", "t2")]))]))]})
    );
}

#[test]
fn closable_false_blocks_close_but_not_removal_of_a_gone_terminal() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([
        {"id":"p1","type":"block","content":{"terminal":"t1"},"closable":false},
        term("p2", "t2"),
    ]))]))]}));
    let before = d.clone();
    assert_eq!(d.close_pane("p1"), Err(Error::NotClosable("p1".into())));
    assert_eq!(d, before);
    assert_eq!(d.close_pane("tb1"), Err(Error::NoPane("tb1".into())));
    let closed = d.remove_gone(|a| a == "t1");
    assert_eq!(closed.len(), 1);
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([{"id":"p2","type":"block","content":{"terminal":"t2"},"size":{"weight":2.0}}])
    );
}

#[test]
fn swap_moves_contents_and_keeps_sizes_in_place() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([
        tab("tb1", json!([
            {"id":"p1","type":"block","content":{"terminal":"t1"},"size":{"px":100.0},"title":"one"},
            {"id":"s1","type":"column","size":{"weight":2.0},"children":[term("p2", "t2"), term("p3", "t3")]},
        ])),
        tab("tb2", json!([term("p4", "t4")])),
    ]))]}));
    d.swap_panes("p1", "p3").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([
            {"id":"p3","type":"block","content":{"terminal":"t3"},"size":{"px":100.0}},
            {"id":"s1","type":"column","size":{"weight":2.0},"children":[
                term("p2", "t2"),
                {"id":"p1","type":"block","content":{"terminal":"t1"},"title":"one"},
            ]},
        ])
    );
    assert_eq!(d.swap_panes("p1", "p1"), Err(Error::SamePane("p1".into())));
    assert_eq!(d.swap_panes("p1", "p4"), Err(Error::OtherTab("p4".into())));
    assert_eq!(d.swap_panes("p1", "s1"), Err(Error::NoPane("s1".into())));
}

#[test]
fn pane_titles_are_trimmed_and_empty_removes_them() {
    let mut d = two_spaces();
    d.set_pane_title("p2", "  server ").unwrap();
    assert_eq!(
        root(&d, "tb1")["children"][1],
        json!({"id":"p2","type":"block","content":{"terminal":"t2"},"title":"server"})
    );
    d.set_pane_title("p2", " ").unwrap();
    assert_eq!(d, two_spaces());
    assert_eq!(
        d.set_pane_title("tb1", "x"),
        Err(Error::NoPane("tb1".into()))
    );
}

fn place(space: &str, tab: Option<&str>) -> Place {
    Place {
        space: space.into(),
        tab: tab.map(str::to_owned),
    }
}

#[test]
fn place_comes_from_non_empty_labels() {
    assert_eq!(
        Place::from_labels(&json!({"herdr.space":"sp1","herdr.tab":"tb1"})),
        Some(place("sp1", Some("tb1")))
    );
    assert_eq!(
        Place::from_labels(&json!({"herdr.space":"sp1","herdr.tab":""})),
        Some(place("sp1", None))
    );
    assert_eq!(
        Place::from_labels(&json!({"herdr.space":"","herdr.tab":"tb1"})),
        None
    );
    assert_eq!(Place::from_labels(&json!({"where":"a / b"})), None);
}

#[test]
fn adopting_into_an_unknown_space_makes_it_unnamed_in_the_live_folder() {
    let mut d = two_spaces();
    let adopted = d
        .adopt("t9", &place("sp9", Some("tb9")), "/live", None)
        .unwrap();
    let Adopted::NewSpace { space, tab, pane } = adopted else {
        panic!("{adopted:?}")
    };
    assert_eq!((space.as_str(), tab.as_str()), ("sp9", "tb9"));
    assert!(is_id(&pane, Prefix::Pane));
    assert_eq!(
        d.to_value()["spaces"][2],
        json!({"id":"sp9","name":"","folder":"/live","tabs":[{"id":"tb9","name":"","root":{"id":"tb9","type":"row","children":[term(&pane, "t9")]}}]})
    );
    let Adopted::NewSpace { tab, .. } = d.adopt("t8", &place("sp8", None), "/x", None).unwrap()
    else {
        panic!()
    };
    assert!(is_id(&tab, Prefix::Tab));
    let Adopted::NewSpace { tab, .. } = d
        .adopt("t7", &place("sp7", Some("tb1")), "/x", None)
        .unwrap()
    else {
        panic!()
    };
    assert!(is_id(&tab, Prefix::Tab));
}

#[test]
fn adopting_into_a_known_tab_splits_its_last_pane_right() {
    let mut d = two_spaces();
    let adopted = d
        .adopt("t9", &place("sp1", Some("tb1")), "/live", None)
        .unwrap();
    let Adopted::Split {
        tab,
        pane,
        container: None,
    } = adopted
    else {
        panic!("{adopted:?}")
    };
    assert_eq!(tab, "tb1");
    assert_eq!(
        root(&d, "tb1")["children"],
        json!([
            term("p1", "t1"),
            {"id":"p2","type":"block","content":{"terminal":"t2"},"size":{"weight":0.5}},
            {"id": pane,"type":"block","content":{"terminal":"t9"},"size":{"weight":0.5}},
        ])
    );
}

#[test]
fn adopting_into_a_known_space_adds_a_tab_after_the_shown_one() {
    let mut d = two_spaces();
    let Adopted::NewTab { tab, pane } = d
        .adopt("t9", &place("sp1", Some("tb9")), "/live", Some("tb1"))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(tab, "tb9");
    assert_eq!(
        d.space("sp1").unwrap().tabs[1],
        Tab::new("tb9", "", Node::terminal(pane, "t9"))
    );
    let Adopted::NewTab { tab, .. } = d.adopt("t8", &place("sp1", None), "/live", None).unwrap()
    else {
        panic!()
    };
    assert_eq!(d.space("sp1").unwrap().tabs.last().unwrap().id, tab);
}

#[test]
fn adopting_a_terminal_that_has_a_pane_changes_nothing() {
    let mut d = two_spaces();
    assert_eq!(
        d.adopt("t3", &place("sp2", None), "/", None).unwrap(),
        Adopted::Present { pane: "p3".into() }
    );
    assert_eq!(d, two_spaces());
}

#[test]
fn adopting_into_a_tab_with_no_panes_fills_it() {
    let mut d = doc(json!({"spaces": [space("sp1", json!([tab("tb1", json!([]))]))]}));
    let Adopted::Split { pane, .. } = d
        .adopt("t9", &place("sp1", Some("tb1")), "/", None)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(root(&d, "tb1")["children"], json!([term(&pane, "t9")]));
}

#[test]
fn gone_terminals_lose_their_panes_tabs_and_spaces() {
    let mut d = two_spaces();
    let closed = d.remove_gone(|a| matches!(a, "t2" | "t4"));
    assert_eq!(
        closed,
        [
            Closed {
                panes: strings(&["p2"]),
                terminals: strings(&["t2"]),
                ..Closed::default()
            },
            Closed {
                panes: strings(&["p4"]),
                tabs: strings(&["tb3"]),
                space: Some("sp2".into()),
                terminals: strings(&["t4"])
            },
        ]
    );
    assert_eq!(d.spaces.len(), 1);
    assert_eq!(d.remove_gone(|_| false), []);
}
