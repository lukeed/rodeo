mod client;
mod env;
mod error;
mod methods;
mod model;
mod paths;
mod start;
mod stream;
mod terminals;
#[cfg(feature = "testing")]
mod testing;
mod wire;

pub use client::{Client, DEFAULT_TIMEOUT, GENERATION, METHODS};
pub use env::Env;
pub use error::{HcError, Mismatch, RemoteError};
pub use methods::{
    Doc, Exec, ExecParams, Holder, ListParams, MachineView, Put, Read, RunParams, ServerStatus,
    Size, TerminalList,
};
pub use model::{
    Change, CoreInfo, Display, Ending, Event, Foreground, Hello, Notice, Snapshot, Status, Terminal,
};
pub use paths::Paths;
pub use start::{find_hc, hc_version, start_server};
pub use stream::{Attach, AttachOptions, AttachReader, AttachWriter, Events, Filter, Output};
pub use terminals::{Terminals, Update};
#[cfg(feature = "testing")]
pub use testing::TestCore;
pub use wire::WireError;
