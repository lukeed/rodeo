mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::{labels, run};
use hc::{Display, ExecParams, HcError, ListParams, RunParams, Size, TestCore};
use serde_json::{Value, json};

#[tokio::test]
async fn hello_names_the_core_its_methods_and_features() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let hello = client.hello();
    assert_eq!(hello.generation, 1);
    assert!(hello.framed);
    assert!(hello.version.starts_with("0."), "{}", hello.version);
    assert_eq!(hello.core.id.len(), 32, "{:?}", hello.core);
    assert_ne!(hello.core.name, "");
    assert!(hello.has_feature("automatic_input"));
    assert!(hc::METHODS.iter().all(|method| hello.has_method(method)));
    let status = client.status().await.expect("status");
    assert_eq!(status.core, hello.core);
    assert_eq!(status.version, hello.version);
    assert!(status.pid > 0);
}

#[tokio::test]
async fn connecting_starts_a_core_when_none_listens() {
    let core = TestCore::new().await;
    assert!(!core.paths().socket.exists());
    let client = core.client().await;
    assert!(core.paths().socket.exists());
    assert_eq!(
        core.paths().socket,
        core.root().join("run/herdr-core/t.sock")
    );
    let status = client.status().await.expect("status");
    assert_eq!((status.live, status.finished), (0, 0));
}

#[tokio::test]
async fn run_takes_every_param() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let cwd = core.root().to_str().expect("utf8").to_owned();
    let started = client
        .run(&RunParams {
            argv: vec![
                "sh".into(),
                "-c".into(),
                r#"printf 'foo=%s pwd=%s\n' "$FOO" "$PWD"; sleep 30"#.into(),
            ],
            size: Some(Size { cols: 90, rows: 30 }),
            display: Some(Display {
                fg: Some("#102030".into()),
                ..Display::default()
            }),
            name: Some("probe".into()),
            cwd: Some(cwd.clone()),
            env: labels(&[("FOO", "bar")]),
            labels: labels(&[("a", "1"), ("gone", "")]),
            keep: true,
            ..RunParams::default()
        })
        .await
        .expect("run");
    assert!(started.address.starts_with('t'));
    assert!(started.is_live());
    assert_eq!(started.name.as_deref(), Some("probe"));
    assert_eq!(started.labels, labels(&[("a", "1")]));
    assert_eq!((started.cols, started.rows, started.keep), (90, 30, true));
    assert_eq!(started.cwd, cwd);
    let read = common::eventually("the program's output", async || {
        let read = client.read("probe", 0).await.expect("read");
        read.text.contains("foo=bar").then_some(read)
    })
    .await;
    assert!(read.text.contains(&format!("pwd={cwd}")), "{}", read.text);

    let taken = client
        .run(&RunParams {
            line: Some("sleep 30".into()),
            name: Some("probe".into()),
            ..RunParams::default()
        })
        .await
        .expect_err("names are unique");
    assert!(taken.is_kind("name_taken"), "{taken}");
}

#[tokio::test]
async fn label_keep_and_kill_change_the_record() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "sleep 30", &[("x", "1")]).await.address;

    let labelled = client
        .label(&t, [("x", ""), ("y", "2")])
        .await
        .expect("label");
    assert_eq!(labelled.labels, labels(&[("y", "2")]));
    assert!(client.keep(&t, true).await.expect("keep").keep);
    assert!(!client.keep(&t, false).await.expect("keep off").keep);

    let listed = client
        .list(&ListParams {
            labels: labels(&[("y", "2")]),
            ..ListParams::default()
        })
        .await
        .expect("list");
    assert_eq!(
        listed
            .terminals
            .iter()
            .map(|t| t.address.as_str())
            .collect::<Vec<_>>(),
        [t.as_str()]
    );

    let killed = client.kill(&t).await.expect("kill");
    assert!(killed.is_finished());
    assert_eq!(killed.reason.as_deref(), Some("killed"));
    assert!(client.get(&t).await.expect("get").is_finished());
    let live = client.list(&ListParams::default()).await.expect("list");
    assert_eq!(live.terminals, []);
    assert_eq!(live.hidden_finished, 1);
    let all = client
        .list(&ListParams {
            all: true,
            ..ListParams::default()
        })
        .await
        .expect("list all");
    assert_eq!(all.terminals.len(), 1);

    let refused = client.keep(&t, true).await.expect_err("finished");
    assert!(refused.is_kind("invalid_params"), "{refused}");
    let missing = client.get("t999").await.expect_err("no such terminal");
    assert!(missing.is_not_found(), "{missing}");
}

#[tokio::test]
async fn docs_are_versioned_and_conflicts_are_their_own_error() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let empty = client.doc_get("herdr-app", None).await.expect("get");
    assert_eq!((empty.value, empty.version), (Value::Null, 0));

    let layout = json!({"spaces": [{"id": "sp1", "name": "", "folder": "/", "tabs": []}]});
    let first = client
        .doc_set("herdr-app", &layout, 0, None)
        .await
        .expect("set");
    assert_eq!(first, 1);
    let read = client.doc_get("herdr-app", None).await.expect("get");
    assert_eq!((read.value, read.version), (layout.clone(), 1));

    let stale = client
        .doc_set("herdr-app", &json!({}), 0, None)
        .await
        .expect_err("stale version");
    assert!(stale.is_conflict(), "{stale}");
    assert!(matches!(stale, HcError::Conflict(_)));

    let deleted = client
        .doc_set("herdr-app", &Value::Null, 1, None)
        .await
        .expect("delete");
    assert_eq!(deleted, 2);
    let read = client.doc_get("herdr-app", None).await.expect("get");
    assert_eq!((read.value, read.version), (Value::Null, 2));

    let bad = client
        .doc_set("Herdr App", &json!({}), 0, None)
        .await
        .expect_err("bad name");
    assert!(bad.is_kind("invalid_params") && !bad.is_conflict(), "{bad}");
    let unknown = client
        .doc_get("herdr-app", Some("nowhere"))
        .await
        .expect_err("no such machine");
    assert!(unknown.is_machine_unknown(), "{unknown}");
}

#[tokio::test]
async fn put_writes_a_file_beside_the_terminal() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "sleep 30", &[]).await.address;
    let data: Vec<u8> = (0..=255).collect();
    let put = client.put(&t, "shot.png", &data).await.expect("put");
    assert!(put.path.ends_with("shot.png"), "{}", put.path);
    assert_eq!(put.bytes, data.len());
    assert_eq!(std::fs::read(&put.path).expect("file"), data);
    let again = client.put(&t, "shot.png", b"x").await.expect("put again");
    assert_ne!(again.path, put.path);
}

#[tokio::test]
async fn exec_returns_code_and_decoded_output() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let done = client
        .exec(&ExecParams {
            argv: vec![
                "sh".into(),
                "-c".into(),
                "printf out; printf err >&2; exit 3".into(),
            ],
            ..ExecParams::default()
        })
        .await
        .expect("exec");
    assert_eq!(
        (done.code, done.stdout.as_slice(), done.stderr.as_slice()),
        (Some(3), &b"out"[..], &b"err"[..])
    );
    assert!(!done.success() && !done.timed_out);

    let input: Vec<u8> = (0..=255).collect();
    let cwd = core.root().to_str().expect("utf8").to_owned();
    let piped = client
        .exec(&ExecParams {
            line: Some("cat; pwd >&2".into()),
            cwd: Some(cwd.clone()),
            env: BTreeMap::from([("X".into(), "1".into())]),
            input: Some(input.clone()),
            ..ExecParams::default()
        })
        .await
        .expect("exec");
    assert!(piped.success());
    assert_eq!(piped.stdout, input);
    assert_eq!(String::from_utf8_lossy(&piped.stderr).trim(), cwd);

    let slow = client
        .exec(&ExecParams {
            line: Some("sleep 5".into()),
            timeout: Some(Duration::from_millis(200)),
            ..ExecParams::default()
        })
        .await
        .expect("exec");
    assert!(slow.timed_out);
}

#[tokio::test]
async fn a_fresh_core_has_no_machines() {
    let core = TestCore::start().await;
    let client = core.client().await;
    assert_eq!(client.machines().await.expect("machines"), []);
}

#[tokio::test]
async fn machine_calls_name_an_unknown_machine() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let rename = client.machine_rename("nope", "other").await;
    assert_eq!(rename.expect_err("rename").kind(), Some("machine_unknown"));
    let rm = client.machine_rm("nope").await;
    assert_eq!(rm.expect_err("rm").kind(), Some("machine_unknown"));
    let connect = client.machine_connect("nope").await;
    assert_eq!(
        connect.expect_err("connect").kind(),
        Some("machine_unknown")
    );
}

#[tokio::test]
async fn a_slow_call_times_out_and_the_client_goes_on() {
    let core = TestCore::start().await;
    let client = core.client().await;
    let t = run(&client, "sleep 30", &[]).await.address;
    let failed = client
        .call_within::<Value>(
            "terminal.wait",
            json!({"terminal": t}),
            Duration::from_millis(300),
        )
        .await
        .expect_err("wait outlasts the timeout");
    assert!(failed.is_timeout(), "{failed}");
    let quick = client.clone().with_timeout(Duration::from_millis(300));
    let failed = quick
        .call::<Value>("terminal.wait", json!({"terminal": t}))
        .await
        .expect_err("wait outlasts the timeout");
    assert!(failed.is_timeout(), "{failed}");
    assert_eq!(quick.timeout(), Duration::from_millis(300));
    assert_eq!(client.timeout(), hc::DEFAULT_TIMEOUT);
    assert!(client.get(&t).await.expect("get").is_live());
}
