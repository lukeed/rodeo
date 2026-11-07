use std::time::Duration;

use hc::{AttachOptions, Client, Filter, HcError, METHODS, Mismatch, Notice, Output};
use serde_json::{Value, json};
use tokio::io::{
    AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf,
};

const DATA: u8 = 0;
const CREDIT: u8 = 1;
const CLOSE: u8 = 2;
const RESIZE: u8 = 3;
const AUTOMATIC_INPUT: u8 = 6;
const WITHIN: Duration = Duration::from_secs(5);

struct Fake {
    read: BufReader<ReadHalf<DuplexStream>>,
    write: WriteHalf<DuplexStream>,
}

#[derive(Debug)]
struct Frame {
    channel: u32,
    kind: u8,
    payload: Vec<u8>,
}

fn hello(features: &[&str]) -> Value {
    json!({
        "generation": 1, "version": "0.1.18", "core": {"id": "c0ffee", "name": "fake"},
        "framed": true, "methods": METHODS, "features": features,
    })
}

async fn connect_with(reply: Value) -> (Result<Client, HcError>, Fake) {
    let (near, far) = tokio::io::duplex(1 << 20);
    let (read, write) = tokio::io::split(far);
    let mut fake = Fake {
        read: BufReader::new(read),
        write,
    };
    let answer = async {
        let mut line = String::new();
        fake.read.read_line(&mut line).await.expect("hello line");
        let request: Value = serde_json::from_str(&line).expect("hello json");
        assert_eq!(request["method"], "hello");
        assert_eq!(request["params"], json!({"generation": 1, "framed": true}));
        let mut reply = reply.clone();
        reply["jsonrpc"] = "2.0".into();
        reply["id"] = request["id"].clone();
        let mut line = serde_json::to_vec(&reply).expect("reply");
        line.push(b'\n');
        fake.write.write_all(&line).await.expect("write hello");
    };
    let (client, ()) = tokio::join!(Client::connect(near), answer);
    (client, fake)
}

async fn connect(features: &[&str]) -> (Client, Fake) {
    let (client, fake) = connect_with(json!({"result": hello(features)})).await;
    (client.expect("hello"), fake)
}

impl Fake {
    async fn send(&mut self, channel: u32, kind: u8, payload: &[u8]) {
        let mut frame = channel.to_le_bytes().to_vec();
        frame.push(kind);
        frame.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        frame.extend_from_slice(payload);
        self.write.write_all(&frame).await.expect("send frame");
    }

    async fn frame(&mut self) -> Frame {
        tokio::time::timeout(WITHIN, async {
            let mut header = [0; 9];
            self.read.read_exact(&mut header).await.expect("header");
            let len = u32::from_le_bytes(header[5..9].try_into().unwrap()) as usize;
            let mut payload = vec![0; len];
            self.read.read_exact(&mut payload).await.expect("payload");
            Frame {
                channel: u32::from_le_bytes(header[..4].try_into().unwrap()),
                kind: header[4],
                payload,
            }
        })
        .await
        .expect("a frame from the client")
    }

    async fn request(&mut self) -> Value {
        let frame = self.frame().await;
        assert_eq!((frame.channel, frame.kind), (0, DATA), "{frame:?}");
        serde_json::from_slice(&frame.payload).expect("json")
    }

    async fn reply(&mut self, request: &Value, result: Value) {
        let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": result});
        self.send(0, DATA, &serde_json::to_vec(&reply).unwrap())
            .await;
    }

    async fn fail(&mut self, request: &Value, kind: &str) {
        let reply = json!({"jsonrpc": "2.0", "id": request["id"],
            "error": {"code": -32000, "message": "no", "data": {"kind": kind, "hint": "try"}}});
        self.send(0, DATA, &serde_json::to_vec(&reply).unwrap())
            .await;
    }
}

fn record(address: &str) -> Value {
    json!({"address": address, "state": "live", "argv": [], "cwd": "/", "cols": 80, "rows": 24, "created_ms": 1})
}

async fn attached(client: &Client, fake: &mut Fake, channel: u32, automatic: bool) -> hc::Attach {
    let options = AttachOptions::new("t1", 80, 24);
    let (attach, ()) = tokio::join!(client.attach(&options), async {
        let request = fake.request().await;
        assert_eq!(request["method"], "terminal.attach");
        fake.reply(
            &request,
            json!({"channel": channel, "terminal": record("t1"), "resize_notifications": false, "automatic_input": automatic}),
        )
        .await;
    });
    attach.expect("attach")
}

#[tokio::test]
async fn hello_is_kept_and_gated() {
    let (client, _fake) = connect(&["framed", "automatic_input"]).await;
    let hello = client.hello();
    assert_eq!(
        (hello.core.name.as_str(), hello.version.as_str()),
        ("fake", "0.1.18")
    );
    assert!(hello.has_feature("automatic_input"));

    let mut newer = hello_value();
    newer["generation"] = 2.into();
    let (refused, _fake) = connect_with(json!({"result": newer})).await;
    assert!(matches!(
        refused,
        Err(HcError::Unsupported(Mismatch::Generation(Some(2))))
    ));

    let (refused, _fake) = connect_with(
        json!({"error": {"code": -32000, "message": "no", "data": {"kind": "unsupported_generation"}}}),
    )
    .await;
    assert!(matches!(
        refused,
        Err(HcError::Unsupported(Mismatch::Generation(None)))
    ));

    let mut thin = hello_value();
    thin["methods"] = json!(["hello", "terminal.run"]);
    let (refused, _fake) = connect_with(json!({"result": thin})).await;
    let Err(HcError::Unsupported(Mismatch::Methods(missing))) = refused else {
        panic!("missing methods refused");
    };
    assert!(missing.contains(&"doc.set".to_owned()));
}

fn hello_value() -> Value {
    hello(&["framed"])
}

#[tokio::test]
async fn replies_match_their_calls_out_of_order() {
    let (client, mut fake) = connect(&[]).await;
    let first = client.call::<Value>("terminal.get", json!({"terminal": "t1"}));
    let second = client.call::<Value>("terminal.get", json!({"terminal": "t2"}));
    let core = async {
        let a = fake.request().await;
        let b = fake.request().await;
        fake.reply(&b, json!({"which": b["params"]["terminal"]}))
            .await;
        fake.fail(&a, "not_found").await;
    };
    let (first, second, ()) = tokio::join!(first, second, core);
    assert!(first.expect_err("t1 fails").is_not_found());
    assert_eq!(second.expect("t2")["which"], "t2");
}

#[tokio::test]
async fn early_frames_are_replayed_once_the_reply_names_the_channel() {
    let (client, mut fake) = connect(&[]).await;
    let event = br#"{"jsonrpc":"2.0","method":"event","params":{"seq":2,"type":"title","terminal":"t1","data":{"title":"x"}}}"#;
    let filter = Filter::default();
    let (subscribed, ()) = tokio::join!(client.subscribe(&filter), async {
        let request = fake.request().await;
        assert_eq!(request["method"], "events.subscribe");
        fake.send(5, DATA, &event[..40]).await;
        fake.send(5, DATA, &[&event[40..], &b"\n"[..]].concat())
            .await;
        fake.reply(
            &request,
            json!({"seq": 1, "terminals": [record("t1")], "channel": 5}),
        )
        .await;
    });
    let (snapshot, mut events) = subscribed.expect("subscribe");
    assert_eq!((snapshot.seq, snapshot.terminals.len()), (1, 1));
    let Some(Ok(Notice::Event(early))) = events.next().await else {
        panic!("the early event");
    };
    assert_eq!(early.seq, 2);
    let credits: Vec<u32> = [fake.frame().await, fake.frame().await]
        .iter()
        .map(|frame| {
            assert_eq!((frame.channel, frame.kind), (5, CREDIT));
            u32::from_le_bytes(frame.payload[..].try_into().unwrap())
        })
        .collect();
    assert_eq!(credits.iter().sum::<u32>() as usize, event.len() + 1);
}

#[tokio::test]
async fn unknown_frame_kinds_are_ignored() {
    let (client, mut fake) = connect(&[]).await;
    let mut attach = attached(&client, &mut fake, 3, false).await;
    fake.send(0, 9, b"?").await;
    fake.send(3, 42, b"?").await;
    fake.send(3, DATA, b"after").await;
    assert_eq!(attach.next().await, Some(Output::Bytes("after".into())));
    let (got, ()) = tokio::join!(client.call::<Value>("server.status", json!({})), async {
        let credit = fake.frame().await;
        assert_eq!((credit.channel, credit.kind), (3, CREDIT));
        let request = fake.request().await;
        fake.reply(&request, json!({"ok": true})).await;
    });
    assert_eq!(got.expect("still answers")["ok"], true);
}

#[tokio::test]
async fn automatic_input_needs_the_feature_and_the_attach_result() {
    for (features, granted, kind) in [
        (&["automatic_input"][..], true, AUTOMATIC_INPUT),
        (&["automatic_input"][..], false, DATA),
        (&[][..], true, DATA),
    ] {
        let (client, mut fake) = connect(features).await;
        let attach = attached(&client, &mut fake, 4, granted).await;
        assert_eq!(
            attach.writer().automatic_input_supported(),
            kind == AUTOMATIC_INPUT
        );
        attach
            .writer()
            .automatic_input(&b"\x1b[?1;2c"[..])
            .expect("send");
        attach.writer().input(&b""[..]).expect("nothing to send");
        attach.writer().input(&b"k"[..]).expect("send");
        let reply = fake.frame().await;
        assert_eq!(
            (reply.channel, reply.kind, &reply.payload[..]),
            (4, kind, &b"\x1b[?1;2c"[..])
        );
        let typed = fake.frame().await;
        assert_eq!((typed.kind, &typed.payload[..]), (DATA, &b"k"[..]));
    }
}

#[tokio::test]
async fn the_writer_sends_resize_focus_facts_and_close() {
    let (client, mut fake) = connect(&[]).await;
    let attach = attached(&client, &mut fake, 6, false).await;
    let (mut reader, writer) = attach.split();
    writer.resize(300, 40).expect("resize");
    writer.focus(true).expect("focus");
    writer
        .facts(&hc::Display {
            fg: Some("#abcdef".into()),
            ..hc::Display::default()
        })
        .expect("facts");
    writer.close().expect("close");
    writer.close().expect("closing twice sends once");
    let frames = [
        fake.frame().await,
        fake.frame().await,
        fake.frame().await,
        fake.frame().await,
    ];
    let kinds: Vec<_> = frames.iter().map(|f| (f.channel, f.kind)).collect();
    assert_eq!(kinds, [(6, RESIZE), (6, 4), (6, 5), (6, CLOSE)]);
    assert_eq!(frames[0].payload, [44, 1, 40, 0]);
    assert_eq!(frames[1].payload, [1]);
    assert_eq!(frames[2].payload, br##"{"fg":"#abcdef"}"##);
    fake.send(6, CLOSE, b"").await;
    assert_eq!(
        reader.next().await,
        Some(Output::Closed(hc::Ending::default()))
    );
    assert_eq!(reader.next().await, None);
    drop((reader, writer));
    fake.send(0, DATA, b"{}").await;
    let (got, ()) = tokio::join!(client.call::<Value>("server.status", json!({})), async {
        let next = fake.frame().await;
        assert_eq!(
            (next.channel, next.kind),
            (0, DATA),
            "no second CLOSE: {next:?}"
        );
        let request: Value = serde_json::from_slice(&next.payload).unwrap();
        fake.reply(&request, json!(1)).await;
    });
    assert_eq!(got.expect("status"), json!(1));
}

#[tokio::test]
async fn core_resizes_and_endings_reach_the_reader() {
    let (client, mut fake) = connect(&[]).await;
    let mut attach = attached(&client, &mut fake, 8, false).await;
    fake.send(8, RESIZE, &[100, 0, 30, 0]).await;
    fake.send(
        8,
        CLOSE,
        br#"{"reason":"exited","exit_code":3,"signal":null,"finished_ms":9}"#,
    )
    .await;
    assert_eq!(
        attach.next().await,
        Some(Output::Resize {
            cols: 100,
            rows: 30
        })
    );
    let Some(Output::Closed(ending)) = attach.next().await else {
        panic!("closed");
    };
    assert_eq!(
        (ending.reason.as_deref(), ending.exit_code),
        (Some("exited"), Some(3))
    );
    assert!(ending.is_exit() && !ending.is_detach());
    drop(attach);
    let (got, ()) = tokio::join!(client.call::<Value>("server.status", json!({})), async {
        let request = fake.request().await;
        fake.reply(&request, json!(2)).await;
    });
    assert_eq!(got.expect("no CLOSE after the core closed"), json!(2));
}

#[tokio::test]
async fn dropping_an_attach_closes_its_channel() {
    let (client, mut fake) = connect(&[]).await;
    let attach = attached(&client, &mut fake, 9, false).await;
    drop(attach);
    let close = fake.frame().await;
    assert_eq!(
        (close.channel, close.kind, close.payload.len()),
        (9, CLOSE, 0)
    );
}

#[tokio::test]
async fn a_call_times_out_cancels_and_the_client_keeps_working() {
    let (client, mut fake) = connect(&[]).await;
    let client = client.with_timeout(Duration::from_millis(100));
    let failed = client
        .call::<Value>("machine.exec", json!({"line": "sleep 9"}))
        .await
        .expect_err("times out");
    assert!(failed.is_timeout(), "{failed}");
    let request = fake.request().await;
    let cancel = fake.request().await;
    assert_eq!(cancel["method"], "$/cancelRequest");
    assert_eq!(cancel["params"]["id"], request["id"]);
    assert!(cancel.get("id").is_none());
    fake.reply(&request, json!("late")).await;

    let (got, ()) = tokio::join!(client.call::<Value>("server.status", json!({})), async {
        let request = fake.request().await;
        fake.reply(&request, json!("now")).await;
    });
    assert_eq!(got.expect("answers"), json!("now"));
}

#[tokio::test]
async fn an_attach_given_up_on_is_closed_when_its_reply_comes() {
    let (client, mut fake) = connect(&[]).await;
    let client = client.with_timeout(Duration::from_millis(100));
    let failed = client
        .attach(&AttachOptions::new("t1", 80, 24))
        .await
        .expect_err("times out");
    assert!(failed.is_timeout());
    let request = fake.request().await;
    let _cancel = fake.request().await;
    fake.send(11, DATA, b"paint").await;
    fake.reply(
        &request,
        json!({"channel": 11, "terminal": record("t1"), "automatic_input": true}),
    )
    .await;
    let close = fake.frame().await;
    assert_eq!((close.channel, close.kind), (11, CLOSE));
}

#[tokio::test]
async fn a_lost_connection_fails_calls_and_ends_streams() {
    let (client, mut fake) = connect(&[]).await;
    let mut attach = attached(&client, &mut fake, 2, false).await;
    let (failed, ()) = tokio::join!(
        client.call::<Value>("terminal.kill", json!({"terminal": "t1"})),
        async {
            let _request = fake.request().await;
            drop(std::mem::replace(&mut fake, dead()));
        }
    );
    assert!(failed.expect_err("lost").is_closed());
    assert_eq!(attach.next().await, None);
    assert!(client.is_closed());
    let after = client.call::<Value>("server.status", json!({})).await;
    assert!(after.expect_err("closed").is_closed());
}

fn dead() -> Fake {
    let (_near, far) = tokio::io::duplex(64);
    let (read, write) = tokio::io::split(far);
    Fake {
        read: BufReader::new(read),
        write,
    }
}
