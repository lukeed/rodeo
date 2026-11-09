use std::any::Any;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hc::ExecParams;

use super::App;
use crate::ids::split;
use crate::overlay::CloseTarget;
use crate::overlay::worktree::{self, Action, Repo, Worktree, WorktreeOp};

const LOOK: &str = r#"cd "$1" 2>/dev/null || exit 3
printf '%s\n' "$HOME"
git rev-parse --show-toplevel || exit 4
git rev-parse --path-format=absolute --git-common-dir || exit 4
git worktree list --porcelain"#;
const ADD: &str = r#"if git -C "$1" show-ref --verify --quiet "refs/heads/$3"; then
  git -C "$2" worktree add "$4" "$3"
else
  git -C "$2" worktree add -b "$3" "$4" HEAD
fi"#;
const SLOW: Duration = Duration::from_secs(600);
const NOT_GIT: &str = "This workspace is not in a Git repository.";
const FROM_PARENT: &str = "New and open worktree actions start from the repo parent workspace.";
const NOT_LINKED: &str = "This workspace is not a Herdr-managed worktree checkout.";
const NONE_FOUND: &str = "No Git worktrees found for this repo.";

pub fn parse_look(out: &str) -> Option<Repo> {
    let mut lines = out.splitn(4, '\n');
    let home = lines.next()?.trim_end().to_owned();
    let top = lines.next()?.trim_end().to_owned();
    let common = lines.next()?.trim_end().trim_end_matches('/');
    let main = common.strip_suffix("/.git").unwrap_or(common).to_owned();
    let checkouts = worktree::parse_porcelain(lines.next().unwrap_or(""));
    (!top.is_empty() && !main.is_empty()).then_some(Repo {
        home,
        top,
        main,
        checkouts,
    })
}

fn git_env() -> BTreeMap<String, String> {
    BTreeMap::from([("LC_ALL".to_owned(), "C".to_owned())])
}

fn failure(exec: &hc::Exec, what: &str) -> String {
    let stderr = String::from_utf8_lossy(&exec.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&exec.stdout).trim().to_owned();
    [stderr, stdout]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{what} failed"))
}

impl App {
    pub fn worktree_dialog(&mut self, id: Option<u64>) -> Option<&mut Worktree> {
        let overlay = self.view.overlay.as_mut()?;
        let any: &mut dyn Any = overlay.as_mut();
        any.downcast_mut::<Worktree>()
            .filter(|w| id.is_none_or(|id| w.id() == id))
    }

    pub fn overlay_opened(&mut self) {
        let Some(dialog) = self.worktree_dialog(None) else {
            return;
        };
        if !dialog.start() {
            return;
        }
        let (id, action, space) = (dialog.id(), dialog.action(), dialog.space().to_owned());
        self.look(id, action, &space);
    }

    fn close_dialog(&mut self, id: u64) {
        if self.worktree_dialog(Some(id)).is_some() {
            self.view.overlay = None;
        }
    }

    fn reject(&mut self, id: u64, error: &str) {
        self.close_dialog(id);
        self.show_error(error);
    }

    fn look(&mut self, id: u64, action: Action, space: &str) {
        let (Some(client), Some((machine, cwd))) = (self.client(), self.space_cwd(space)) else {
            self.reject(id, NOT_GIT);
            return;
        };
        let places: Vec<(String, String)> = self
            .machines
            .spaces()
            .iter()
            .filter(|s| s.machine == machine.as_deref())
            .filter_map(|s| Some((s.key(), self.space_cwd(&s.key())?.1)))
            .collect();
        let params = ExecParams {
            argv: vec!["sh".into(), "-c".into(), LOOK.into(), "sh".into(), cwd],
            on: machine,
            env: git_env(),
            timeout: Some(Duration::from_secs(30)),
            ..ExecParams::default()
        };
        self.spawn(async move {
            let result = client.exec(&params).await;
            move |app: &mut App| {
                let repo = match result {
                    Ok(exec) if exec.success() => {
                        parse_look(&String::from_utf8_lossy(&exec.stdout))
                    }
                    Ok(exec) if matches!(exec.code, Some(3 | 4)) => None,
                    Ok(exec) => {
                        app.reject(id, &failure(&exec, "git worktree list"));
                        return;
                    }
                    Err(error) => {
                        app.reject(id, &error.to_string());
                        return;
                    }
                };
                app.looked_up(id, action, repo, &places);
            }
        });
    }

    fn looked_up(
        &mut self,
        id: u64,
        action: Action,
        repo: Option<Repo>,
        places: &[(String, String)],
    ) {
        let Some(repo) = repo else {
            self.reject(id, NOT_GIT);
            return;
        };
        match action {
            Action::New | Action::Open if repo.linked() => return self.reject(id, FROM_PARENT),
            Action::Remove if !repo.linked() => return self.reject(id, NOT_LINKED),
            Action::Open if worktree::entries(&repo, places).is_empty() => {
                return self.reject(id, NONE_FOUND);
            }
            _ => {}
        }
        let directory = self.config.worktrees.directory.clone();
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
        if let Some(dialog) = self.worktree_dialog(Some(id)) {
            dialog.load(&repo, &directory, places, seed);
        }
    }

    pub fn worktree_op(&mut self, op: WorktreeOp) {
        match op {
            WorktreeOp::Create {
                dialog,
                space,
                branch,
                path,
                top,
                main,
            } => self.create_worktree(dialog, &space, branch, path, top, main),
            WorktreeOp::Open {
                dialog,
                space,
                path,
                shown_in,
            } => {
                self.close_dialog(dialog);
                if let Some(shown) = shown_in.filter(|s| self.machines.space(s).is_some()) {
                    self.focus_space(&shown);
                } else {
                    let machine = split(&space).0.map(str::to_owned);
                    self.new_space_in(machine, path, String::new());
                }
            }
            WorktreeOp::Remove {
                dialog,
                space,
                path,
                main,
                force,
            } => self.remove_worktree(dialog, space, path, main, force),
        }
    }

    fn create_worktree(
        &mut self,
        id: u64,
        space: &str,
        branch: String,
        path: String,
        top: String,
        main: String,
    ) {
        let Some(client) = self.client() else {
            return self.worktree_failed(id, "hc is not connected".into());
        };
        let machine = split(space).0.map(str::to_owned);
        let params = ExecParams {
            argv: vec![
                "sh".into(),
                "-c".into(),
                ADD.into(),
                "sh".into(),
                main,
                top,
                branch,
                path.clone(),
            ],
            on: machine.clone(),
            env: git_env(),
            timeout: Some(SLOW),
            ..ExecParams::default()
        };
        self.spawn(async move {
            let result = client.exec(&params).await;
            move |app: &mut App| match result {
                Ok(exec) if exec.success() => {
                    app.close_dialog(id);
                    app.new_space_in(machine, path, String::new());
                }
                Ok(exec) => app.worktree_failed(id, failure(&exec, "git worktree add")),
                Err(error) => app.worktree_failed(id, error.to_string()),
            }
        });
    }

    fn remove_worktree(&mut self, id: u64, space: String, path: String, main: String, force: bool) {
        let Some(client) = self.client() else {
            return self.worktree_failed(id, "hc is not connected".into());
        };
        let mut argv = vec![
            "git".into(),
            "-C".into(),
            main,
            "worktree".into(),
            "remove".into(),
        ];
        if force {
            argv.push("--force".into());
        }
        argv.push(path);
        let params = ExecParams {
            argv,
            on: split(&space).0.map(str::to_owned),
            env: git_env(),
            timeout: Some(SLOW),
            ..ExecParams::default()
        };
        self.spawn(async move {
            let result = client.exec(&params).await;
            move |app: &mut App| match result {
                Ok(exec) if exec.success() => {
                    app.close_dialog(id);
                    app.close(&CloseTarget::Workspace { space });
                }
                Ok(exec) => {
                    let message = failure(&exec, "git worktree remove");
                    if !force && worktree::is_force_needed(&message) {
                        if let Some(dialog) = app.worktree_dialog(Some(id)) {
                            dialog.needs_force();
                        }
                    } else {
                        app.worktree_failed(id, message);
                    }
                }
                Err(error) => app.worktree_failed(id, error.to_string()),
            }
        });
    }

    fn worktree_failed(&mut self, id: u64, error: String) {
        if let Some(dialog) = self.worktree_dialog(Some(id)) {
            dialog.failed(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_look;

    #[test]
    fn look_output_gives_the_repo() {
        let repo = parse_look(
            "/h\n/w/wt/repo/a\n/r/.git\nworktree /r\nbranch refs/heads/main\n\nworktree /w/wt/repo/a\nbranch refs/heads/a\n",
        )
        .expect("a repo");
        assert_eq!(repo.home, "/h");
        assert_eq!(repo.main, "/r");
        assert!(repo.linked());
        assert_eq!(repo.name(), "r");
        assert_eq!(repo.checkouts.len(), 2);
        assert!(parse_look("/h\n").is_none());
    }
}
