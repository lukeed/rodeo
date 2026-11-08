mod common;

use common::{Harness, run};

const HIDE: &str = "printf '\\033[?25l'; echo hidden-now\r";

async fn hidden(h: &mut Harness) {
    h.typed(HIDE).await;
    let address = h.focused_address().expect("a pane");
    h.until("the cursor hidden", |h| {
        h.pane_text(&address).contains("\nhidden-now")
            && !h
                .app()
                .panes
                .get(&address)
                .expect("pane")
                .term
                .cursor()
                .visible
    })
    .await;
    h.screen();
}

fn bar_shown(h: &mut Harness) -> bool {
    h.screen();
    h.host_output().contains("\x1b[6 q")
}

#[test]
fn a_hidden_cursor_stays_hidden_by_default() {
    run(|| async {
        let mut h =
            Harness::with_config("[experimental]\ncjk_ime_cursor_shape = \"steady_bar\"\n").await;
        h.ready().await;
        hidden(&mut h).await;
        assert!(!bar_shown(&mut h));
    });
}

#[test]
fn the_ime_option_reveals_the_hidden_cursor_with_its_shape() {
    run(|| async {
        let mut h = Harness::with_config(
            "[experimental]\nreveal_hidden_cursor_for_cjk_ime = true\ncjk_ime_cursor_shape = \"steady_bar\"\n",
        )
        .await;
        h.ready().await;
        hidden(&mut h).await;
        assert!(bar_shown(&mut h));
    });
}

#[test]
fn the_agent_filter_skips_other_programs() {
    run(|| async {
        let mut h = Harness::with_config(
            "[experimental]\nreveal_hidden_cursor_for_cjk_ime = true\ncjk_ime_agents = [\"claude\"]\ncjk_ime_cursor_shape = \"steady_bar\"\n",
        )
        .await;
        h.ready().await;
        hidden(&mut h).await;
        assert!(!bar_shown(&mut h), "a plain shell is not claude");
    });
}
