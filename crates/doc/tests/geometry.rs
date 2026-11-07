use std::collections::HashMap;

use doc::{
    Axis, Cell, Direction, Document, Error, RESIZE_STEP, Rect, Size, Slot, neighbor, split_lengths,
    split_rect,
};
use serde_json::{Value, json};

fn weight(w: f32) -> Slot {
    Slot {
        size: Some(Size::Weight { weight: w }),
        ..Slot::default()
    }
}

fn px(p: f32) -> Slot {
    Slot {
        size: Some(Size::Px { px: p }),
        ..Slot::default()
    }
}

const FLEX: Slot = Slot {
    size: None,
    min: None,
    max: None,
};

#[test]
fn weights_split_into_cells_that_sum_exactly() {
    assert_eq!(split_lengths(100, &[FLEX, FLEX], 1.0), [50, 50]);
    assert_eq!(split_lengths(101, &[FLEX, FLEX], 1.0), [51, 50]);
    assert_eq!(split_lengths(100, &[FLEX, FLEX, FLEX], 1.0), [34, 33, 33]);
    assert_eq!(
        split_lengths(10, &[weight(0.5), weight(0.25), weight(0.25)], 1.0),
        [5, 3, 2]
    );
    assert_eq!(
        split_lengths(7, &[weight(1.0 / 3.0), weight(2.0 / 3.0)], 1.0),
        [2, 5]
    );
    assert_eq!(split_lengths(0, &[FLEX, FLEX], 1.0), [0, 0]);
    assert_eq!(split_lengths(5, &[], 1.0), Vec::<u16>::new());
    assert_eq!(
        split_lengths(3, &[FLEX, FLEX, FLEX, FLEX, FLEX], 1.0),
        [1, 1, 1, 0, 0]
    );
}

#[test]
fn px_sizes_are_fixed_cells_and_weights_share_the_rest() {
    assert_eq!(
        split_lengths(100, &[px(160.0), FLEX, FLEX], 8.0),
        [20, 40, 40]
    );
    assert_eq!(split_lengths(100, &[px(160.0), px(80.0)], 8.0), [67, 33]);
    assert_eq!(split_lengths(10, &[px(800.0), FLEX], 8.0), [10, 0]);
    let min = Slot {
        min: Some(400.0),
        ..FLEX
    };
    assert_eq!(split_lengths(100, &[min, FLEX, FLEX], 8.0), [50, 25, 25]);
    let max = Slot {
        max: Some(80.0),
        ..FLEX
    };
    assert_eq!(split_lengths(100, &[max, FLEX], 8.0), [10, 90]);
}

#[test]
fn odd_weights_never_break_the_sum() {
    let odd = [
        weight(0.0),
        weight(-1.0),
        weight(f32::NAN),
        weight(f32::INFINITY),
    ];
    assert_eq!(split_lengths(9, &odd, 1.0).iter().sum::<u16>(), 9);
    for total in [0u16, 1, 2, 3, 17, 80, 211, 1000, u16::MAX] {
        for slots in [
            vec![FLEX],
            vec![weight(0.1), weight(0.7), weight(0.2)],
            vec![px(33.0), weight(2.0), FLEX, px(7.0)],
            vec![weight(1e-6), weight(1e6)],
        ] {
            let lengths = split_lengths(total, &slots, 7.0);
            assert_eq!(
                lengths.iter().map(|&c| u32::from(c)).sum::<u32>(),
                u32::from(total),
                "{total} {slots:?}"
            );
        }
    }
}

#[test]
fn rects_split_along_their_axis() {
    let area = Rect::new(2, 3, 81, 20);
    let cell = Cell {
        width: 8,
        height: 16,
    };
    assert_eq!(
        split_rect(area, Axis::Row, &[FLEX, FLEX], cell),
        [Rect::new(2, 3, 41, 20), Rect::new(43, 3, 40, 20)]
    );
    assert_eq!(
        split_rect(area, Axis::Column, &[px(160.0), FLEX], cell),
        [Rect::new(2, 3, 81, 10), Rect::new(2, 13, 81, 10)]
    );
}

#[test]
fn neighbor_picks_the_nearest_overlapping_pane() {
    let panes = [
        ("a", Rect::new(0, 0, 50, 20)),
        ("b", Rect::new(50, 0, 50, 10)),
        ("c", Rect::new(50, 10, 50, 10)),
        ("d", Rect::new(0, 20, 100, 20)),
    ];
    assert_eq!(neighbor("a", Direction::Right, &panes), Some("b"));
    assert_eq!(neighbor("a", Direction::Down, &panes), Some("d"));
    assert_eq!(neighbor("a", Direction::Left, &panes), None);
    assert_eq!(neighbor("a", Direction::Up, &panes), None);
    assert_eq!(neighbor("c", Direction::Up, &panes), Some("b"));
    assert_eq!(neighbor("c", Direction::Left, &panes), Some("a"));
    assert_eq!(neighbor("d", Direction::Up, &panes), Some("a"));
    assert_eq!(neighbor("z", Direction::Up, &panes), None);
}

#[test]
fn neighbor_ties_go_to_larger_overlap_then_closer_center_then_order() {
    let from = ("f", Rect::new(10, 10, 10, 10));
    let small = ("small", Rect::new(0, 10, 10, 2));
    let large = ("large", Rect::new(0, 10, 10, 8));
    assert_eq!(
        neighbor("f", Direction::Left, &[from, small, large]),
        Some("large")
    );
    let high = ("high", Rect::new(0, 4, 10, 10));
    let low = ("low", Rect::new(0, 16, 10, 4));
    assert_eq!(
        neighbor("f", Direction::Left, &[from, high, low]),
        Some("low")
    );
    let one = ("one", Rect::new(0, 10, 10, 10));
    let two = ("two", Rect::new(0, 10, 10, 10));
    assert_eq!(
        neighbor("f", Direction::Left, &[from, one, two]),
        Some("one")
    );
    let far = ("far", Rect::new(30, 10, 10, 10));
    let near = ("near", Rect::new(20, 15, 5, 2));
    assert_eq!(
        neighbor("f", Direction::Right, &[far, from, near]),
        Some("near")
    );
}

fn tab(children: Value) -> Document {
    let mut value =
        json!({"spaces":[{"id":"sp","tabs":[{"id":"t","root":{"id":"t","type":"row"}}]}]});
    value["spaces"][0]["tabs"][0]["root"]["children"] = children;
    serde_json::from_value(value).unwrap()
}

fn term(id: &str) -> Value {
    json!({"id": id, "type":"block","content":{"terminal": id}})
}

fn rects(items: &[(&str, Rect)]) -> impl Fn(&str) -> Option<Rect> + use<> {
    let map: HashMap<String, Rect> = items.iter().map(|(k, r)| ((*k).to_owned(), *r)).collect();
    move |id| map.get(id).copied()
}

const CELL: Cell = Cell {
    width: 8,
    height: 16,
};

fn size(d: &Document, id: &str) -> Option<Size> {
    d.tab("t").unwrap().root.find(id).unwrap().size
}

fn weight_of(d: &Document, id: &str) -> f32 {
    match size(d, id) {
        Some(Size::Weight { weight }) => weight,
        None => 1.0,
        other => panic!("{other:?}"),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

fn halves() -> (Document, impl Fn(&str) -> Option<Rect>) {
    let d = tab(json!([term("a"), term("b")]));
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 50, 40)),
        ("b", Rect::new(50, 0, 50, 40)),
    ]);
    (d, r)
}

#[test]
fn resize_moves_the_divider_in_the_direction_like_herdr() {
    let (mut d, r) = halves();
    assert!(
        d.resize_pane("a", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "a"), 1.1) && close(weight_of(&d, "b"), 0.9));
    let (mut d, r) = halves();
    assert!(
        d.resize_pane("b", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "a"), 1.1) && close(weight_of(&d, "b"), 0.9));
    let (mut d, r) = halves();
    assert!(
        d.resize_pane("a", Direction::Left, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "a"), 0.9) && close(weight_of(&d, "b"), 1.1));
    let (mut d, r) = halves();
    assert!(
        !d.resize_pane("a", Direction::Up, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert_eq!(d, halves().0);
}

#[test]
fn resize_keeps_the_pair_sum_and_leaves_other_siblings_alone() {
    let mut d = tab(json!([
        {"id":"a","type":"block","content":{"terminal":"a"},"size":{"weight":0.5}},
        {"id":"b","type":"block","content":{"terminal":"b"},"size":{"weight":0.25}},
        {"id":"c","type":"block","content":{"terminal":"c"},"size":{"weight":0.25}},
    ]));
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 50, 40)),
        ("b", Rect::new(50, 0, 25, 40)),
        ("c", Rect::new(75, 0, 25, 40)),
    ]);
    assert!(
        d.resize_pane("b", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "a"), 0.5));
    assert!(close(weight_of(&d, "b"), 0.3) && close(weight_of(&d, "c"), 0.2));
}

#[test]
fn resize_falls_back_to_an_ancestor_divider() {
    let mut d = tab(json!([
        {"id":"col","type":"column","children":[term("p1"), term("p2")]},
        term("p3"),
    ]));
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("col", Rect::new(0, 0, 60, 40)),
        ("p1", Rect::new(0, 0, 60, 20)),
        ("p2", Rect::new(0, 20, 60, 20)),
        ("p3", Rect::new(60, 0, 40, 40)),
    ]);
    assert!(
        d.resize_pane("p1", Direction::Left, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "col"), 2.0 * 55.0 / 100.0));
    assert!(close(weight_of(&d, "p3"), 2.0 * 45.0 / 100.0));
    assert_eq!(size(&d, "p1"), None);
    assert!(
        d.resize_pane("p1", Direction::Down, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "p1"), 2.0 * 22.0 / 40.0));
}

#[test]
fn resize_stops_at_a_tenth_of_the_pair() {
    let (mut d, r) = halves();
    assert!(d.resize_pane("a", Direction::Left, 0.5, &r, CELL).unwrap());
    assert!(close(weight_of(&d, "a"), 0.2) && close(weight_of(&d, "b"), 1.8));
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 10, 40)),
        ("b", Rect::new(10, 0, 90, 40)),
    ]);
    assert!(
        !d.resize_pane("a", Direction::Left, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
}

#[test]
fn resize_keeps_px_sizes_in_px() {
    let mut d = tab(
        json!([{"id":"a","type":"block","content":{"terminal":"a"},"size":{"px":200.0}}, term("b")]),
    );
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 25, 40)),
        ("b", Rect::new(25, 0, 75, 40)),
    ]);
    assert!(
        d.resize_pane("b", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert_eq!(size(&d, "a"), Some(Size::Px { px: 240.0 }));
    assert_eq!(size(&d, "b"), None);
    let mut d = tab(
        json!([term("a"), {"id":"b","type":"block","content":{"terminal":"b"},"size":{"px":600.0}}]),
    );
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 25, 40)),
        ("b", Rect::new(25, 0, 75, 40)),
    ]);
    assert!(
        d.resize_pane("a", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert_eq!(size(&d, "b"), Some(Size::Px { px: 560.0 }));
    assert_eq!(size(&d, "a"), None);
}

#[test]
fn resize_needs_rects_and_a_resizable_container() {
    let (mut d, _) = halves();
    assert_eq!(
        d.resize_pane("a", Direction::Right, RESIZE_STEP, |_| None, CELL),
        Err(Error::Unmeasured("t".into()))
    );
    assert_eq!(
        d.resize_pane("t", Direction::Right, RESIZE_STEP, |_| None, CELL),
        Err(Error::NoPane("t".into()))
    );
    let mut d = tab(
        json!([{"id":"s","type":"row","resizable":false,"children":[term("a"), term("b")]}, term("c")]),
    );
    let r = rects(&[
        ("s", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 50, 40)),
        ("b", Rect::new(50, 0, 50, 40)),
    ]);
    assert!(
        !d.resize_pane("a", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
}

#[test]
fn resize_skips_hidden_siblings() {
    let mut d = tab(
        json!([term("a"), {"id":"h","type":"block","content":{"terminal":"h"},"hidden":true}, term("b")]),
    );
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 50, 40)),
        ("b", Rect::new(50, 0, 50, 40)),
    ]);
    assert!(
        d.resize_pane("a", Direction::Right, RESIZE_STEP, &r, CELL)
            .unwrap()
    );
    assert!(close(weight_of(&d, "a"), 1.1) && close(weight_of(&d, "b"), 0.9));
    assert_eq!(size(&d, "h"), None);
}

#[test]
fn divider_drag_rescales_the_pair() {
    let (mut d, r) = halves();
    assert!(d.drag_divider("t", 0, 30.0, &r, CELL).unwrap());
    assert!(close(weight_of(&d, "a"), 0.6) && close(weight_of(&d, "b"), 1.4));
    let (mut d, r) = halves();
    assert!(d.drag_divider("t", 0, 2.0, &r, CELL).unwrap());
    assert!(close(weight_of(&d, "a"), 0.2));
    let (mut d, r) = halves();
    assert_eq!(
        d.drag_divider("t", 1, 2.0, &r, CELL),
        Err(Error::NoDivider("t".into(), 1))
    );
    assert_eq!(
        d.drag_divider("a", 0, 2.0, &r, CELL),
        Err(Error::NoNode("a".into()))
    );
    assert_eq!(
        d.drag_divider("zz", 0, 2.0, &r, CELL),
        Err(Error::NoNode("zz".into()))
    );
}

#[test]
fn divider_drag_honors_min_and_max() {
    let mut d = tab(
        json!([{"id":"a","type":"block","content":{"terminal":"a"},"min":400.0,"max":560.0}, term("b")]),
    );
    let r = rects(&[
        ("t", Rect::new(0, 0, 100, 40)),
        ("a", Rect::new(0, 0, 50, 40)),
        ("b", Rect::new(50, 0, 50, 40)),
    ]);
    d.drag_divider("t", 0, 20.0, &r, CELL).unwrap();
    assert!(close(weight_of(&d, "a"), 1.0));
    d.drag_divider("t", 0, 90.0, &r, CELL).unwrap();
    assert!(close(weight_of(&d, "a"), 1.4));
}
