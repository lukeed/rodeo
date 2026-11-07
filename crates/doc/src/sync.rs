use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use crate::schema::Document;

pub const NAME: &str = "herdr-app";
pub const MAX_BYTES: usize = 1024 * 1024;

impl Document {
    pub fn parse(value: &Value) -> Result<Document, serde_json::Error> {
        if value.is_null() {
            return Ok(Document::default());
        }
        Document::deserialize(value)
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("a document always serializes")
    }

    pub fn to_json(&self) -> String {
        self.to_value().to_string()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Write {
    pub value: Value,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Planned<T> {
    pub doc: Document,
    pub write: Option<Write>,
    pub output: T,
}

#[derive(Debug)]
pub enum PlanError<E> {
    Undecodable(serde_json::Error),
    Edit(E),
    Unwritable,
    TooLarge(usize),
}

impl<E: fmt::Display> fmt::Display for PlanError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::Undecodable(e) => write!(f, "cannot read the layout: {e}"),
            PlanError::Edit(e) => e.fmt(f),
            PlanError::Unwritable => f.write_str("the edit made a layout the app cannot read"),
            PlanError::TooLarge(n) => write!(
                f,
                "the layout is {n} bytes, over the {MAX_BYTES} byte limit"
            ),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for PlanError<E> {}

pub fn plan<T, E>(
    value: &Value,
    version: u64,
    edit: impl FnOnce(&mut Document) -> Result<T, E>,
) -> Result<Planned<T>, PlanError<E>> {
    let before = Document::parse(value).map_err(PlanError::Undecodable)?;
    let mut doc = before.clone();
    let output = edit(&mut doc).map_err(PlanError::Edit)?;
    if doc == before {
        return Ok(Planned {
            doc,
            write: None,
            output,
        });
    }
    let value = doc.to_value();
    if Document::parse(&value).ok().as_ref() != Some(&doc) {
        return Err(PlanError::Unwritable);
    }
    let bytes = serde_json::to_vec(&value)
        .expect("a value always serializes")
        .len();
    if bytes > MAX_BYTES {
        return Err(PlanError::TooLarge(bytes));
    }
    Ok(Planned {
        doc,
        write: Some(Write { value, version }),
        output,
    })
}
