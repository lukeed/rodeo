use std::any::Any;
use std::collections::BTreeMap;

use config::PopupSize;

use super::App;
use crate::clipboard::shell_quote;
use crate::overlay::{MachineManager, MachineOp};
use crate::popup::Spec;

const KEEP_ON_ERROR: &str = r#""$@"
status=$?
if [ "$status" -ne 0 ]; then
  printf '\nPress enter to close.'
  read -r _
fi
exit "$status""#;

struct Run {
    title: String,
    args: Vec<String>,
    picked: Option<String>,
    seek: Option<String>,
}

impl App {
    pub fn open_machines(&mut self, picked: Option<String>) {
        self.set_overlay(Box::new(MachineManager::new(picked)));
        self.poll_machines();
    }

    pub fn machine_manager(&mut self) -> Option<&mut MachineManager> {
        let overlay = self.view.overlay.as_mut()?;
        let any: &mut dyn Any = overlay.as_mut();
        any.downcast_mut::<MachineManager>()
    }

    fn machine_finished(&mut self, result: Result<Option<String>, String>) {
        if let Err(error) = &result {
            tracing::warn!(%error, "machine call failed");
        }
        match self.machine_manager() {
            Some(manager) => manager.finished(result),
            None => {
                if let Err(error) = result {
                    self.show_error(error);
                }
            }
        }
    }

    fn machine_called(
        &mut self,
        views: Result<Vec<hc::MachineView>, hc::HcError>,
        result: Result<Option<String>, String>,
    ) {
        self.machines_listed(views);
        self.machine_finished(result);
    }

    pub fn machine_op(&mut self, op: MachineOp) {
        let Some(client) = self.client() else {
            self.machine_finished(Err("hc is not connected".to_owned()));
            return;
        };
        match op {
            MachineOp::Rename { machine, name } => self.spawn(async move {
                let result = client
                    .machine_rename(&machine, &name)
                    .await
                    .map(|_| None)
                    .map_err(|e| format!("cannot rename {machine}: {e}"));
                let views = client.machines().await;
                move |app: &mut App| app.machine_called(views, result)
            }),
            MachineOp::Remove { machine } => self.spawn(async move {
                let result = client
                    .machine_rm(&machine)
                    .await
                    .map(|()| Some(format!("removed {machine}")))
                    .map_err(|e| format!("cannot remove {machine}: {e}"));
                let views = client.machines().await;
                move |app: &mut App| app.machine_called(views, result)
            }),
            MachineOp::Connect { machine } => self.spawn(async move {
                let result = client
                    .machine_connect(&machine)
                    .await
                    .map(|view| Some(format!("{machine}: {}", view.state)))
                    .map_err(|e| format!("cannot reconnect {machine}: {e}"));
                let views = client.machines().await;
                move |app: &mut App| app.machine_called(views, result)
            }),
            MachineOp::Add { target, name } => {
                let mut args = vec!["machine".to_owned(), "add".to_owned(), target.clone()];
                if let Some(name) = &name {
                    args.extend(["--name".to_owned(), name.clone()]);
                }
                self.machine_popup(Run {
                    title: "add machine".to_owned(),
                    args,
                    seek: name.is_none().then_some(target),
                    picked: name,
                });
            }
            MachineOp::Login { machine } => self.machine_popup(Run {
                title: format!("log in to {machine}"),
                args: vec!["machine".into(), "login".into(), machine.clone()],
                picked: Some(machine),
                seek: None,
            }),
            MachineOp::Update { machine } => self.machine_popup(Run {
                title: format!("update {machine}"),
                args: vec!["machine".into(), "update".into(), machine.clone()],
                picked: Some(machine),
                seek: None,
            }),
        }
    }

    fn machine_popup(&mut self, run: Run) {
        let paths = match hc::Paths::resolve(&self.env) {
            Ok(paths) => paths,
            Err(error) => {
                self.machine_finished(Err(error.to_string()));
                return;
            }
        };
        let env = self.env.clone();
        self.spawn(async move {
            let bin = hc::find_hc(&env).await;
            move |app: &mut App| match bin {
                Some(bin) => app.run_machine_popup(run, &bin, &paths),
                None => app.machine_finished(Err(hc::HcError::NoBinary.to_string())),
            }
        });
    }

    fn run_machine_popup(&mut self, run: Run, bin: &std::path::Path, paths: &hc::Paths) {
        let shown: Vec<String> = run
            .args
            .iter()
            .map(|arg| {
                if arg.contains(char::is_whitespace) {
                    shell_quote(arg)
                } else {
                    arg.clone()
                }
            })
            .collect();
        let command = format!("hc {}", shown.join(" "));
        let mut argv = vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            KEEP_ON_ERROR.to_owned(),
            "sh".to_owned(),
            bin.to_string_lossy().into_owned(),
        ];
        argv.extend(run.args);
        let env = BTreeMap::from([
            (
                "HC_SOCKET".to_owned(),
                paths.socket.to_string_lossy().into_owned(),
            ),
            ("HC_SESSION".to_owned(), paths.session.clone()),
        ]);
        let (picked, seek) = (run.picked, run.seek);
        let spec = Spec {
            argv,
            machine: None,
            cwd: Some(self.home()),
            env,
            title: run.title,
            width: Some(PopupSize::Percent(80)),
            height: Some(PopupSize::Percent(70)),
            after: Some(Box::new(move |app: &mut App, exit: Option<i32>| {
                let error = exit
                    .filter(|code| *code != 0)
                    .map(|code| format!("{command} exited with code {code}"));
                let reopen = move |app: &mut App| {
                    if app.view.overlay.is_none() && app.popup.is_none() {
                        let manager = MachineManager::new(picked).seeking(seek).with_error(error);
                        app.set_overlay(Box::new(manager));
                    } else if let Some(error) = error {
                        app.show_error(error);
                    }
                };
                let Some(client) = app.client() else {
                    reopen(app);
                    return;
                };
                app.spawn(async move {
                    let views = client.machines().await;
                    move |app: &mut App| {
                        app.machines_listed(views);
                        reopen(app);
                    }
                });
            })),
        };
        if let Err(error) = self.start_popup(spec) {
            self.machine_finished(Err(error.to_owned()));
        }
    }
}
