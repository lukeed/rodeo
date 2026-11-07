mod common;

use common::{WITHIN, Watch, labels, run, sized};
use hc::{
    AttachOptions, Change, Event, Filter, ListParams, Notice, RunParams, Size, Terminal, Terminals,
    TestCore, Update,
};
use serde_json::{Value, json};

fn kind(event: &Event) -> &'static str {
    match event.change {
        Change::Created(_) => "created",
        Change::Lost(_) => "lost",
        Change::Restarted(_) => "restarted",
        Change::Keep { .. } => "keep",
        Change::Finished(_) => "finished",
        Change::Removed {} => "removed",
        Change::Status { .. } => "status",
        Change::Turn { .. } => "turn",
        Change::Title { .. } => "title",
        Change::Labels { .. } => "labels",
        Change::Size { .. } => "size",
        Change::Foreground(_) => "foreground",
        Change::Doc { .. } => "doc",
        Change::Unknown { .. } => "unknown",
    }
}

#[tokio::test]
async fn events_arrive_in_order() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let mut watch = Watch::start(&client, &Filter::default()).await;
    let t = client
        .run(&RunParams {
            line: Some("sleep 30".into()),
            size: Some(Size { cols: 80, rows: 24 }),
            ..RunParams::default()
        })
        .await
        .expect("run")
        .address;
    let attach = client
        .attach(&AttachOptions::new(&t, 100, 30))
        .await
        .expect("attach");
    watch.until("attach size", sized(100, 30)).await;
    attach.writer().resize(120, 40).expect("resize");
    watch.until("resize", sized(120, 40)).await;
    drop(attach);
    watch.until("headless", sized(200, 120)).await;
    client.label(&t, [("x", "1")]).await.expect("label");
    client.keep(&t, true).await.expect("keep");
    client.kill(&t).await.expect("kill");
    let seen = watch
        .until("finished", |e| matches!(e.change, Change::Finished(_)))
        .await;
    let kinds: Vec<_> = seen
        .iter()
        .map(kind)
        .filter(|kind| *kind != "foreground")
        .collect();
    assert_eq!(kinds, ["labels", "keep", "finished"]);
    assert!(
        seen.iter()
            .all(|e| e.terminal.as_deref() == Some(t.as_str()))
    );
    assert!(seen.iter().all(|e| e.machine.is_none()));
}

#[tokio::test]
async fn the_first_events_follow_the_snapshot() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let before = run(&client, "sleep 30", &[]).await.address;
    let (snapshot, mut events) = client
        .subscribe(&Filter::default())
        .await
        .expect("subscribe");
    assert_eq!(
        snapshot
            .terminals
            .iter()
            .map(|t| t.address.as_str())
            .collect::<Vec<_>>(),
        [before.as_str()]
    );
    let after = run(&client, "sleep 30", &[]).await.address;
    let first = tokio::time::timeout(WITHIN, async {
        loop {
            if let Notice::Event(event) = events.next().await.expect("open").expect("notice") {
                return event;
            }
        }
    })
    .await
    .expect("an event");
    assert_eq!(first.seq, snapshot.seq + 1);
    assert_eq!(first.terminal.as_deref(), Some(after.as_str()));
}

#[tokio::test]
async fn doc_events_reach_only_unfiltered_subscribers() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let mut all = Watch::start(&client, &Filter::default()).await;
    let mut tagged = Watch::start(
        &client,
        &Filter {
            labels: labels(&[("x", "1")]),
            ..Filter::default()
        },
    )
    .await;
    let version = client
        .doc_set("herdr-app", &json!({"spaces": []}), 0, None)
        .await
        .expect("set");
    let seen = all
        .until("doc", |e| matches!(e.change, Change::Doc { .. }))
        .await;
    let doc = seen.last().expect("doc");
    assert_eq!(
        doc.change,
        Change::Doc {
            name: "herdr-app".into(),
            version
        }
    );
    assert_eq!(
        (doc.terminal.as_deref(), doc.machine.as_deref()),
        (None, None)
    );

    let t = run(&client, "sleep 30", &[("x", "1")]).await.address;
    let seen = tagged
        .until("created", |e| matches!(e.change, Change::Created(_)))
        .await;
    assert_eq!(seen.iter().map(kind).collect::<Vec<_>>(), ["created"]);
    assert_eq!(seen[0].terminal.as_deref(), Some(t.as_str()));
}

#[tokio::test]
async fn folding_events_gives_what_the_core_lists() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let filter = Filter {
        labels: labels(&[("proof", "fold")]),
        ..Filter::default()
    };
    let (snapshot, mut events) = client.subscribe(&filter).await.expect("subscribe");
    let mut view = Terminals::new(snapshot, &filter);
    let tag = [("proof", "fold")];
    let agent = run(
        &client,
        r"printf '\033]7501;state=working:progress=10\033\\\033]0;agent\007'; sleep 0.2
          printf '\033]7501;state=done:msg=b2s=\033\\'; sleep 30",
        &tag,
    )
    .await
    .address;
    let quick = run(&client, "exit 4", &tag).await.address;
    let gone = run(&client, "true", &tag).await.address;
    let leaving = run(&client, "sleep 30", &tag).await.address;
    let _: Value = client
        .call("terminal.wait", json!({"terminal": gone}))
        .await
        .expect("wait");
    let _: Value = client
        .call("terminal.rm", json!({"terminal": gone}))
        .await
        .expect("rm");
    let _: Value = client
        .call("terminal.wait", json!({"terminal": agent}))
        .await
        .expect("wait");
    client
        .label(&leaving, [("proof", "")])
        .await
        .expect("label");
    let last = run(&client, "sleep 30", &tag).await.address;

    let mut updates = Vec::new();
    tokio::time::timeout(WITHIN, async {
        while view.get(&last).is_none() {
            let notice = events.next().await.expect("open").expect("notice");
            updates.push(view.apply(notice));
        }
    })
    .await
    .expect("events up to the last terminal");

    let listed = client
        .list(&ListParams {
            all: true,
            labels: labels(&[("proof", "fold")]),
        })
        .await
        .expect("list");
    let facts = |t: &Terminal| {
        (
            t.address.clone(),
            t.state.clone(),
            t.status.clone(),
            t.turn,
            t.title.clone(),
            t.exit_code,
            t.reason.clone(),
            t.labels.clone(),
        )
    };
    let mut want: Vec<_> = listed.terminals.iter().map(facts).collect();
    let mut have: Vec<_> = view.iter().map(facts).collect();
    want.sort_by(|a, b| a.0.cmp(&b.0));
    have.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(have, want);
    assert!(view.get(&gone).is_none() && view.get(&leaving).is_none());
    assert!(updates.contains(&Update::Removed(gone.clone())));
    assert!(updates.contains(&Update::Removed(leaving.clone())));
    assert_eq!(view.get(&quick).and_then(|t| t.exit_code), Some(4));
    let status = view
        .get(&agent)
        .and_then(|t| t.status.clone())
        .expect("status");
    assert_eq!((status.state.as_str(), status.msg.as_str()), ("done", "ok"));
}

#[tokio::test]
async fn a_kept_terminal_comes_back_after_a_signal() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let mut watch = Watch::start(&client, &Filter::default()).await;
    let t = client
        .run(&RunParams {
            line: Some("printf ready; sleep 300".into()),
            keep: true,
            ..RunParams::default()
        })
        .await
        .expect("run")
        .address;
    let mut attach = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect("attach");
    let mut screen = common::Screen::default();
    screen.shows(&mut attach, "ready").await;
    let pid = client.get(&t).await.expect("get").pid.expect("pid");
    let killed = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .expect("kill");
    assert!(killed.success());

    let ended = screen.ends(&mut attach).await;
    assert_eq!(
        (ended.reason.as_deref(), ended.signal),
        (Some("signaled"), Some(9))
    );
    let seen = watch
        .until("restarted", |e| matches!(e.change, Change::Restarted(_)))
        .await;
    assert!(seen.iter().any(|e| matches!(e.change, Change::Lost(_))));

    let mut again = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect("reattach");
    again
        .writer()
        .input(&b"echo back$((1+1))\r"[..])
        .expect("input");
    common::Screen::default().shows(&mut again, "back2").await;
}
