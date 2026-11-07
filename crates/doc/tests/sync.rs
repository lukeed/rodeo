use doc::{Document, Error, MAX_BYTES, NAME, Node, PlanError, Size, Tab, Write, plan};
use serde_json::{Value, json};

fn fetched() -> Value {
    json!({"spaces":[{"id":"sp1","name":"","folder":"/w","tabs":[
        {"id":"tb1","name":"","root":{"id":"tb1","type":"row","children":[
            {"id":"p1","type":"block","content":{"terminal":"t1"}},
        ]}},
    ]}]})
}

#[test]
fn the_doc_is_named_like_the_apps() {
    assert_eq!(NAME, "herdr-app");
}

#[test]
fn an_edit_becomes_a_write_at_the_fetched_version() {
    let planned = plan(&fetched(), 7, |d| d.rename_tab("tb1", " build ")).unwrap();
    let Some(Write { value, version }) = planned.write else {
        panic!("no write")
    };
    assert_eq!(version, 7);
    assert_eq!(value["spaces"][0]["tabs"][0]["name"], "build");
    assert_eq!(Document::parse(&value).unwrap(), planned.doc);
}

#[test]
fn an_edit_that_changes_nothing_is_no_write() {
    let planned = plan(&fetched(), 7, |d| d.rename_tab("tb1", "  ")).unwrap();
    assert_eq!(planned.write, None);
    let planned = plan(&fetched(), 7, |d| d.step_tab("tb1", true)).unwrap();
    assert_eq!(planned.write, None);
    let planned = plan(&fetched(), 7, |_| Ok::<_, Error>(())).unwrap();
    assert_eq!(planned.write, None);
}

#[test]
fn unknown_fields_alone_never_cause_a_write() {
    let mut value = fetched();
    value["spaces"][0]["extra"] = json!(true);
    let planned = plan(&value, 3, |d| d.rename_space("sp1", "")).unwrap();
    assert_eq!(planned.write, None);
}

#[test]
fn an_empty_doc_starts_at_version_zero() {
    let planned = plan(&Value::Null, 0, |d| {
        d.new_space(
            "sp1",
            "",
            "/w",
            Tab::new("tb1", "", Node::terminal("p1", "t1")),
        )
    })
    .unwrap();
    let write = planned.write.unwrap();
    assert_eq!((write.value, write.version), (fetched(), 0));
}

#[test]
fn edit_errors_and_bad_docs_are_reported() {
    let err = plan(&fetched(), 1, |d| d.close_pane("nope")).unwrap_err();
    assert!(matches!(err, PlanError::Edit(Error::NoPane(ref id)) if id == "nope"));
    assert_eq!(err.to_string(), "no pane \"nope\"");
    let err = plan(&json!({"spaces": 1}), 1, |d| d.close_pane("p1")).unwrap_err();
    assert!(matches!(err, PlanError::Undecodable(_)));
}

#[test]
fn a_value_the_app_cannot_read_is_never_planned() {
    let err = plan(&fetched(), 1, |d| {
        d.tab_mut("tb1").unwrap().root.children_mut().unwrap()[0].size =
            Some(Size::Weight { weight: f32::NAN });
        Ok::<_, Error>(())
    })
    .unwrap_err();
    assert!(matches!(err, PlanError::Unwritable));
}

#[test]
fn a_doc_over_one_mib_is_never_planned() {
    let err = plan(&fetched(), 1, |d| {
        d.rename_space("sp1", &"x".repeat(MAX_BYTES))
    })
    .unwrap_err();
    assert!(matches!(err, PlanError::TooLarge(n) if n > MAX_BYTES));
}

#[test]
fn a_conflict_retry_replays_the_edit_on_the_new_doc() {
    let edit = |d: &mut Document| d.split_pane("p1", doc::Axis::Row, Node::terminal("p2", "t2"));
    let first = plan(&fetched(), 4, edit).unwrap();
    assert_eq!(first.write.as_ref().unwrap().version, 4);
    let mut theirs = fetched();
    theirs["spaces"][0]["name"] = json!("theirs");
    let again = plan(&theirs, 5, edit).unwrap();
    let write = again.write.unwrap();
    assert_eq!(write.version, 5);
    assert_eq!(write.value["spaces"][0]["name"], "theirs");
    assert_eq!(
        write.value["spaces"][0]["tabs"][0]["root"]["children"][1]["id"],
        "p2"
    );
    let landed = plan(&write.value, 6, edit).unwrap_err();
    assert!(matches!(landed, PlanError::Edit(Error::IdTaken(_))));
}
