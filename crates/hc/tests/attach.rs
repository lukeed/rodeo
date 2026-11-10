mod common;

use common::{Screen, WITHIN, Watch, eventually, run, size_of, sized};
use hc::{AttachOptions, Display, Filter, Output, RunParams, Size, TestCore};

const BYTES: &str =
    "stty raw -echo; printf ready; while :; do dd bs=1 count=1 2>/dev/null | od -An -tx1; done";

#[tokio::test]
async fn typed_bytes_reach_the_program_and_its_exit_closes_the_stream() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(
        &client,
        r#"printf 'ready\n'; read x; printf 'got:%s\n' "$x"; exit 3"#,
        &[],
    )
    .await;
    let mut attach = client
        .attach(&AttachOptions::new(&t.address, 80, 24))
        .await
        .expect("attach");
    assert_eq!(attach.terminal.address, t.address);
    let mut screen = Screen::default();
    screen.shows(&mut attach, "ready").await;
    attach.writer().input(&b"hi\r"[..]).expect("input");
    screen.shows(&mut attach, "got:hi").await;
    let ended = screen.ends(&mut attach).await;
    assert_eq!(
        (ended.reason.as_deref(), ended.exit_code),
        (Some("exited"), Some(3))
    );
    assert!(ended.is_exit());
    assert_eq!(attach.next().await, None);
}

#[tokio::test]
async fn automatic_input_reaches_the_program_without_taking_the_size() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let mut watch = Watch::start(&client, &Filter::default()).await;
    let t = client
        .run(&RunParams {
            line: Some(BYTES.into()),
            size: Some(Size { cols: 80, rows: 24 }),
            ..RunParams::default()
        })
        .await
        .expect("run")
        .address;
    let mut desk = client
        .attach(&AttachOptions::new(&t, 100, 30))
        .await
        .expect("desk");
    watch.until("desk size", sized(100, 30)).await;
    let phone = client
        .attach(&AttachOptions::new(&t, 70, 25))
        .await
        .expect("phone");
    watch.until("phone size", sized(70, 25)).await;
    assert!(desk.writer().automatic_input_supported());

    let mut screen = Screen::default();
    screen.shows(&mut desk, "ready").await;
    desk.writer()
        .automatic_input(&b"\x1b[?62c"[..])
        .expect("automatic input");
    screen
        .until(&mut desk, "the reply bytes", |s| {
            s.hex().ends_with(b"\x1b[?62c")
        })
        .await;
    phone.writer().resize(71, 26).expect("resize");
    let seen = watch.until("phone resize", sized(71, 26)).await;
    assert!(
        seen.iter()
            .filter_map(size_of)
            .all(|size| size != (100, 30)),
        "automatic input never takes the size: {seen:#?}"
    );
    desk.writer().input(&b"k"[..]).expect("typing");
    watch.until("typing takes the size", sized(100, 30)).await;
}

#[tokio::test]
async fn resizes_set_the_size_and_reach_observers_that_ask() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "sleep 30", &[]).await.address;
    let viewer = client
        .attach(&AttachOptions::new(&t, 100, 30))
        .await
        .expect("viewer");
    let mut observer = client
        .attach(&AttachOptions {
            observe: true,
            resize_notifications: true,
            ..AttachOptions::new(&t, 60, 20)
        })
        .await
        .expect("observer");
    assert!(observer.resize_notifications);
    viewer.writer().resize(120, 40).expect("resize");
    let mut screen = Screen::default();
    screen
        .until(&mut observer, "a resize notice", |s| {
            s.resizes.contains(&(120, 40))
        })
        .await;
    let got = client.get(&t).await.expect("get");
    assert_eq!((got.cols, got.rows), (120, 40));
}

#[tokio::test]
async fn focus_and_facts_reach_the_core() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(
        &client,
        r"printf '\033[?1004h'; stty raw -echo; printf ready; dd bs=1 count=1 >/dev/null 2>&1; printf '\033]10;?\033\\'; while :; do dd bs=1 count=1 2>/dev/null | od -An -tx1; done",
        &[],
    )
    .await
    .address;
    let mut attach = client
        .attach(&AttachOptions {
            display: Display {
                fg: Some("#102030".into()),
                ..Display::default()
            },
            ..AttachOptions::new(&t, 80, 24)
        })
        .await
        .expect("attach");
    let mut screen = Screen::default();
    screen.shows(&mut attach, "ready").await;
    attach
        .writer()
        .facts(&Display {
            fg: Some("#abcdef".into()),
            ..Display::default()
        })
        .expect("facts");
    attach.writer().input(&b"x"[..]).expect("go");
    screen
        .until(&mut attach, "the color answer", |s| {
            String::from_utf8_lossy(&s.hex()).contains("rgb:abab/cdcd/efef")
        })
        .await;
    attach.writer().focus(true).expect("focus");
    screen
        .until(&mut attach, "focus in", |s| s.hex().ends_with(b"\x1b[I"))
        .await;
    attach.writer().focus(false).expect("blur");
    screen
        .until(&mut attach, "focus out", |s| s.hex().ends_with(b"\x1b[O"))
        .await;
}

#[tokio::test]
async fn closing_gets_an_empty_close_back() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "printf ready; sleep 30", &[]).await.address;
    let mut attach = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect("attach");
    let mut screen = Screen::default();
    screen.shows(&mut attach, "ready").await;
    attach.writer().close().expect("close");
    let ended = screen.ends(&mut attach).await;
    assert!(ended.is_detach(), "{ended:?}");
    assert_eq!(attach.next().await, None);
    assert!(client.get(&t).await.expect("get").is_live());
}

#[tokio::test]
async fn dropping_the_last_viewer_returns_the_headless_size() {
    let core = TestCore::start().await;
    let client = core.client().await;
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
    eventually("the viewer's size", async || {
        let got = client.get(&t).await.expect("get");
        ((got.cols, got.rows) == (100, 30)).then_some(())
    })
    .await;
    drop(attach);
    eventually("the headless size", async || {
        let got = client.get(&t).await.expect("get");
        ((got.cols, got.rows) == (200, 120)).then_some(())
    })
    .await;
}

#[tokio::test]
async fn split_halves_work_from_separate_tasks() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(
        &client,
        r#"printf 'ready\n'; read x; printf 'got:%s\n' "$x"; sleep 30"#,
        &[],
    )
    .await
    .address;
    let attach = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect("attach");
    let (mut reader, writer) = attach.split();
    let reading = tokio::spawn(async move {
        let mut screen = Screen::default();
        screen.shows(&mut reader, "got:split").await;
        reader
    });
    let writing = tokio::spawn(async move {
        writer.input(&b"split\r"[..]).expect("input");
        writer
    });
    let writer = writing.await.expect("writer task");
    let mut reader = tokio::time::timeout(WITHIN, reading)
        .await
        .expect("in time")
        .expect("reader task");
    drop(writer);
    let mut screen = Screen::default();
    let ended = screen.ends(&mut reader).await;
    assert!(ended.is_detach(), "dropping both halves closes: {ended:?}");
}

#[tokio::test]
async fn history_comes_first_only_when_asked() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "seq 1 300; printf done; sleep 30", &[])
        .await
        .address;
    let mut asked = client
        .attach(&AttachOptions {
            history: 1000,
            ..AttachOptions::new(&t, 80, 24)
        })
        .await
        .expect("attach");
    let mut screen = Screen::default();
    screen.shows(&mut asked, "done").await;
    let raw = screen.text();
    assert!(raw.contains("\n150\r") && raw.contains("\n2\r"), "{raw}");

    let mut plain = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect("attach");
    let mut screen = Screen::default();
    screen.shows(&mut plain, "done").await;
    assert!(!screen.text().contains("\n150\r"));
}

#[tokio::test]
async fn a_stalled_stream_never_blocks_calls_and_credit_lets_it_finish() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(
        &client,
        "printf start; head -c 3000000 /dev/zero | tr '\\0' x; printf END; sleep 30",
        &[],
    )
    .await
    .address;
    let mut attach = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect("attach");
    for _ in 0..20 {
        let got = tokio::time::timeout(WITHIN, client.get(&t))
            .await
            .expect("calls answer while the stream waits for credit")
            .expect("get");
        assert_eq!(got.address, t);
    }
    let mut received = Vec::new();
    let ended = tokio::time::timeout(WITHIN, async {
        while let Some(Output::Bytes(bytes)) = attach.next().await {
            received.extend_from_slice(&bytes);
            if received[received.len().saturating_sub(64)..]
                .windows(3)
                .any(|w| w == b"END")
            {
                return;
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "no END after {} bytes", received.len());
}

#[tokio::test]
async fn attach_to_a_finished_terminal_says_finished() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "true", &[]).await.address;
    eventually("it finishes", async || {
        client
            .get(&t)
            .await
            .expect("get")
            .is_finished()
            .then_some(())
    })
    .await;
    let refused = client
        .attach(&AttachOptions::new(&t, 80, 24))
        .await
        .expect_err("finished");
    assert!(refused.is_finished(), "{refused}");
}
