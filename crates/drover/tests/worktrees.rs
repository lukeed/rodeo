mod common;

use std::path::Path;
use std::process::Command;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};

const KEYS: &str = "[keys]\nopen_worktree = \"prefix+u\"\nremove_worktree = \"prefix+y\"\n";

fn git(h: &Harness, dir: &Path, args: &[&str]) -> String {
    let out = h
        .core
        .command("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-C"])
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn make_repo(h: &Harness) -> std::path::PathBuf {
    let repo = h.core.root().join("repo");
    std::fs::create_dir_all(&repo).expect("repo dir");
    git(h, &repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "a").expect("a file");
    git(h, &repo, &["add", "."]);
    git(h, &repo, &["commit", "-qm", "first"]);
    repo
}

async fn cd(h: &mut Harness, dir: &Path) {
    h.typed(&format!("cd {}\r", dir.display())).await;
    let address = h.focused_address().expect("a pane");
    let want = dir.display().to_string();
    h.until("the shell in the folder", |h| {
        h.app()
            .terminals
            .get(&address)
            .and_then(|t| t.foreground.as_ref())
            .is_some_and(|f| f.cwd == want)
    })
    .await;
}

fn worktrees(h: &Harness, repo: &Path) -> String {
    git(h, repo, &["worktree", "list", "--porcelain"])
}

#[test]
fn new_open_and_remove_worktrees() {
    run(|| async {
        let mut h = Harness::with_config(KEYS).await;
        h.ready().await;
        let repo = make_repo(&h);
        cd(&mut h, &repo).await;
        let root = h.core.root().join(".herdr/worktrees/repo");

        h.prefix_char('G').await;
        h.until("the new worktree dialog", |h| {
            h.screen().contains("new worktree")
                && h.screen().contains("/.herdr/worktrees/repo/worktree-")
        })
        .await;
        h.typed("feat/x").await;
        let checkout = root.join("feat-x");
        let shown = checkout.display().to_string();
        h.until("the checkout preview", |h| h.screen().contains(&shown))
            .await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let folder = shown.clone();
        let doc = h
            .until_stored("a space in the checkout", |d| {
                d.spaces.iter().any(|s| s.folder == folder)
            })
            .await;
        assert_eq!(doc.spaces.len(), 2);
        assert!(checkout.join("a.txt").exists());
        assert!(worktrees(&h, &repo).contains("branch refs/heads/feat/x"));
        h.until("the dialog to close", |h| h.app().view.overlay.is_none())
            .await;
        h.until("focus on the new space", |h| active_folder(h) == shown)
            .await;

        h.prefix_char('u').await;
        h.until("the error for a linked checkout", |h| {
            has_error(h, "repo parent workspace")
        })
        .await;

        let other = root.join("other");
        git(
            &h,
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "other",
                &other.display().to_string(),
            ],
        );
        let main_space = doc.spaces[0].id.clone();
        h.app_mut().focus_space(&main_space);
        h.prefix_char('u').await;
        h.until("the open worktree dialog", |h| {
            h.screen().contains("open worktree") && h.screen().contains("3 checkouts")
        })
        .await;
        h.typed("/other").await;
        h.until("the filter", |h| h.screen().contains("1/3 checkouts"))
            .await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let folder = other.display().to_string();
        h.until_stored("a space for the other checkout", |d| {
            d.spaces.len() == 3 && d.spaces.iter().any(|s| s.folder == folder)
        })
        .await;

        h.app_mut().focus_space(&main_space);
        h.prefix_char('u').await;
        h.until("the open worktree dialog", |h| {
            h.screen().contains("open worktree")
        })
        .await;
        h.typed("/feat").await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("focus on the open checkout", |h| active_folder(h) == shown)
            .await;
        assert_eq!(h.doc().spaces.len(), 3);

        cd(&mut h, &checkout).await;
        std::fs::write(checkout.join("dirty.txt"), "x").expect("an untracked file");
        h.prefix_char('y').await;
        h.until("the remove dialog", |h| {
            h.screen().contains("delete worktree checkout?") && h.screen().contains(&shown)
        })
        .await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("the second confirmation", |h| {
            h.screen().contains("delete anyway")
        })
        .await;
        assert!(checkout.exists());
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let folder = shown.clone();
        h.until_stored("the checkout space closed", |d| {
            d.spaces.len() == 2 && d.spaces.iter().all(|s| s.folder != folder)
        })
        .await;
        assert!(!checkout.exists());
        assert!(!worktrees(&h, &repo).contains(&shown));
        assert!(git(&h, &repo, &["branch", "--list", "feat/x"]).contains("feat/x"));

        h.app_mut().focus_space(&main_space);
        h.prefix_char('y').await;
        h.until("the error for the main checkout", |h| {
            has_error(h, "not a Herdr-managed")
        })
        .await;
        assert!(h.app().view.overlay.is_none());
    });
}

fn has_error(h: &Harness, text: &str) -> bool {
    h.app()
        .error
        .as_ref()
        .is_some_and(|(e, _)| e.contains(text))
}

fn active_folder(h: &Harness) -> String {
    let app = h.app();
    app.view
        .space
        .as_deref()
        .and_then(|s| app.machines.space(s))
        .map(|s| s.space.folder.clone())
        .unwrap_or_default()
}

#[test]
fn worktree_actions_need_a_git_space() {
    run(|| async {
        let mut h = Harness::with_config(KEYS).await;
        h.ready().await;
        h.prefix_char('G').await;
        h.until("the error outside git", |h| {
            has_error(h, "not in a Git repository")
        })
        .await;
        assert!(h.app().view.overlay.is_none());
        let repo = make_repo(&h);
        cd(&mut h, &repo).await;
        h.prefix_char('G').await;
        h.until("the dialog", |h| h.screen().contains("new worktree"))
            .await;
        h.press(KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("the empty branch error", |h| {
            h.screen().contains("branch is required")
        })
        .await;
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert!(h.app().view.overlay.is_none());
    });
}
