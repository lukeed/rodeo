mod common;

use common::{Harness, run};
use config::ToastDelivery;
use crossterm::event::Event;
use rodeo::sound::Sound;

const REPORT: &str = r"printf '\033]7501;state=%s:app=claude-code\033\\'";

fn script(states: &[(&str, f32)]) -> String {
    let mut line = "sleep 2".to_owned();
    for (state, wait) in states {
        let report = REPORT.replace("%s", state);
        line = format!("{line}; {report}; sleep {wait}");
    }
    line.push_str("; sleep 60");
    line
}

async fn agent(h: &mut Harness, states: &[(&str, f32)]) -> String {
    let doc = h
        .until_stored("the first space", |d| d.spaces.len() == 1)
        .await;
    let tabs = doc.spaces[0].tabs.len();
    let terminal = h
        .outside
        .run(&hc::RunParams {
            line: Some(script(states)),
            labels: [
                ("herdr.space".to_owned(), doc.spaces[0].id.clone()),
                ("herdr.tab".to_owned(), doc::fresh_id(doc::Prefix::Tab)),
                ("herdr.seen".to_owned(), "0".to_owned()),
            ]
            .into(),
            ..hc::RunParams::default()
        })
        .await
        .expect("an agent");
    h.until_stored("a tab for the agent", |d| {
        d.spaces[0].tabs.len() == tabs + 1
    })
    .await;
    terminal.address
}

fn sounds(h: &Harness) -> Vec<Sound> {
    h.app()
        .notices
        .sounds
        .played
        .iter()
        .map(|(s, _)| *s)
        .collect()
}

fn sent(h: &Harness) -> Vec<(ToastDelivery, String)> {
    h.app()
        .notices
        .sent
        .iter()
        .map(|s| (s.delivery, s.title.clone()))
        .collect()
}

#[test]
fn terminal_delivery_writes_osc_and_plays_sounds() {
    run(|| async {
        let mut h =
            Harness::with_config("[ui.toast]\ndelivery = \"terminal\"\ndelay_seconds = 0\n").await;
        h.ready().await;
        h.app_mut().env.set("TERM_PROGRAM", "ghostty");
        let address = agent(
            &mut h,
            &[
                ("working", 0.5),
                ("blocked", 0.5),
                ("working", 0.5),
                ("idle", 0.5),
            ],
        )
        .await;
        h.until("both notices", |h| sent(h).len() == 2).await;
        assert_eq!(
            sent(&h),
            [
                (ToastDelivery::Terminal, "claude needs attention".to_owned()),
                (ToastDelivery::Terminal, "claude finished".to_owned()),
            ]
        );
        assert_eq!(sounds(&h), [Sound::Request, Sound::Done]);
        let out = h.host_output();
        assert!(out.contains("\x1b]9;claude needs attention: "), "{out:?}");
        assert!(out.contains("\x1b]9;claude finished: "), "{out:?}");
        assert!(h.app().notice.is_none(), "terminal delivery shows no card");
        h.outside.kill(&address).await.expect("kill");
    });
}

#[test]
fn herdr_cards_queue_and_open_their_target() {
    run(|| async {
        let mut h = Harness::with_config(
            "[ui.toast]\ndelivery = \"herdr\"\ndelay_seconds = 0\n[ui.sound]\nenabled = false\n",
        )
        .await;
        h.ready().await;
        let first = agent(&mut h, &[("working", 0.5), ("idle", 0.5)]).await;
        let second = agent(&mut h, &[("working", 0.5), ("blocked", 0.5)]).await;
        h.until("a queued notice", |h| !h.app().notices.queue.is_empty())
            .await;
        assert!(sounds(&h).is_empty(), "sound is off");
        let shown = h.app().notice.clone().expect("a card");
        assert_eq!(shown.title, "claude finished");
        assert!(h.screen().contains("claude finished"));
        h.prefix_char('o').await;
        assert_eq!(h.focused_address().as_deref(), Some(first.as_str()));
        let next = h.app().notice.clone().expect("the queued card");
        assert_eq!(next.title, "claude needs attention");
        h.send(Event::FocusLost);
        h.pump().await;
        h.until("the card on screen", |h| {
            h.screen().contains("claude needs attention")
        })
        .await;
        h.outside.kill(&first).await.expect("kill");
        h.outside.kill(&second).await.expect("kill");
    });
}

#[test]
fn system_delivery_and_agent_sound_overrides() {
    run(|| async {
        let mut h = Harness::with_config(
            "[ui.toast]\ndelivery = \"system\"\ndelay_seconds = 1\n[ui.sound.agents]\nclaude = \"off\"\n",
        )
        .await;
        h.ready().await;
        let address = agent(
            &mut h,
            &[
                ("working", 0.5),
                ("blocked", 0.2),
                ("working", 1.5),
                ("blocked", 0.5),
            ],
        )
        .await;
        h.until("one attention notice", |h| !sent(h).is_empty())
            .await;
        h.settle().await;
        assert_eq!(
            sent(&h),
            [(ToastDelivery::System, "claude needs attention".to_owned())],
            "the first blocked state did not hold for the delay"
        );
        assert!(sounds(&h).is_empty(), "claude sounds are off");
        h.outside.kill(&address).await.expect("kill");
    });
}
