mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};
use rodeo::agents::State;
use rodeo::overlay::goto::{self, Target};
use rodeo::view::Mode;

const REPORT: &str = r"printf '\033]7501;state=%s:app=pi\033\\'";

fn report(state: &str) -> String {
    REPORT.replace("%s", state)
}

#[test]
fn children_show_under_their_agent_and_open_in_a_peek() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let parent = h.focused_address().expect("the first pane");
        h.typed(&format!("{}\r", report("working"))).await;
        h.until("an agent row", |h| h.app().snapshot().agents.len() == 1)
            .await;
        let child = h
            .outside
            .run(&hc::RunParams {
                line: Some(format!("{}; sleep 60", report("blocked"))),
                name: Some("helper".into()),
                parent: Some(parent.clone()),
                ..hc::RunParams::default()
            })
            .await
            .expect("a child")
            .address;
        h.until("the blocked child", |h| {
            h.app()
                .snapshot()
                .children
                .iter()
                .any(|c| c.address == child && c.state == State::Blocked)
        })
        .await;
        let snapshot = h.app().snapshot();
        assert_eq!(snapshot.children.len(), 1);
        assert_eq!(snapshot.children[0].root, parent);
        assert_eq!(snapshot.children[0].name, "helper");
        assert_eq!(snapshot.agents.len(), 1, "a child is not an agent row");
        assert_eq!(snapshot.agents[0].kids, Some(State::Blocked));
        assert_eq!(
            snapshot.spaces[0].state,
            Some(State::Working),
            "children stay out of the rollup"
        );
        assert!(h.app().notices.sent.is_empty(), "children never toast");
        assert!(!h.app().panes.contains(&child), "no pane for a child");

        let screen = h.screen();
        assert!(screen.contains("└─"), "{screen}");
        assert!(screen.contains("helper"), "{screen}");
        let rows = goto::rows(&snapshot, "help", None);
        assert!(
            rows.iter()
                .any(|r| r.target == Target::Child(child.clone())),
            "{rows:?}"
        );

        let row = h
            .app()
            .hits
            .sidebar
            .agents
            .iter()
            .find(|a| a.child.as_deref() == Some(child.as_str()))
            .map(|a| a.rect)
            .expect("a row for the child");
        h.click(row.x + 6, row.y).await;
        h.until("the peek", |h| {
            h.app()
                .popup
                .as_ref()
                .is_some_and(|p| p.peek && p.address().as_deref() == Some(child.as_str()))
        })
        .await;
        h.until("the child on screen", |h| h.screen().contains("q close"))
            .await;
        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
        assert_eq!(h.app().view.mode, Mode::Prefix);
        let screen = h.screen();
        assert!(screen.contains("PREFIX"), "{screen}");
        assert!(screen.contains("send prefix  q close"), "{screen}");
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        assert!(h.app().popup.is_some(), "esc only cancels the prefix");
        h.prefix_char('q').await;
        h.until("the peek closed", |h| h.app().popup.is_none())
            .await;
        h.settle().await;
        assert!(
            h.app()
                .terminals
                .get(&child)
                .is_some_and(hc::Terminal::is_live),
            "closing a peek leaves the child running"
        );
        let _ = h.outside.kill(&child).await;
    });
}
