mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};

#[test]
fn opening_a_space_from_navigate_mode_leaves_no_selection() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nprompt_new_workspace_name = false\n").await;
        h.ready().await;
        h.prefix_char('N').await;
        h.until_stored("two spaces", |d| d.spaces.len() == 2).await;
        h.prefix_char('w').await;
        h.press(KeyCode::Up, KeyModifiers::NONE).await;
        assert!(h.app().snapshot().spaces.iter().any(|s| s.selected));
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let spaces = h.app().snapshot().spaces;
        assert!(spaces[0].focused);
        assert!(spaces.iter().all(|s| !s.selected), "{spaces:?}");
    });
}
