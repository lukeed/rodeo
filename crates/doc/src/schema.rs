use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub spaces: Vec<Space>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub tabs: Vec<Tab>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "empty_root")]
    pub root: Node,
}

fn empty_root() -> Node {
    Node::container("", Axis::Row, Vec::new())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Terminal {
        terminal: String,
    },
    Mod {
        #[serde(rename = "mod")]
        name: String,
        #[serde(default, skip_serializing_if = "Value::is_null")]
        props: Value,
    },
    Context {
        #[serde(alias = "context")]
        tab: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pane: Option<Box<PaneFrame>>,
    },
    Agent {
        agent: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    #[default]
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Float {
    pub id: String,
    pub content: Content,
    #[serde(default)]
    pub edge: Edge,
    #[serde(default)]
    pub align: Align,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peek: Option<f32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Size {
    Px { px: f32 },
    Weight { weight: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    #[serde(default = "one")]
    pub width: f32,
    #[serde(default = "border_color")]
    pub color: String,
}

fn one() -> f32 {
    1.0
}

fn border_color() -> String {
    "border".to_owned()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Background {
    Color(String),
    Gradient {
        gradient: [String; 2],
        #[serde(default = "downward")]
        angle: f32,
    },
    Image {
        image: String,
        #[serde(default)]
        fit: Fit,
    },
}

fn downward() -> f32 {
    180.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Cover,
    Contain,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Padding {
    All(f32),
    Axes([f32; 2]),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Look {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_padding: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_opacity: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Theme {
    One(String),
    Pair { light: String, dark: String },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FocusLook {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Line>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HeaderContent {
    Builtin(String),
    Mod {
        #[serde(rename = "mod")]
        name: String,
        #[serde(default, skip_serializing_if = "Value::is_null")]
        props: Value,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub content: HeaderContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reveal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PaneFrame {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<Header>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Line>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Line>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub lone: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Kind {
    Block {
        content: Content,
    },
    Row {
        children: Vec<Node>,
    },
    Column {
        children: Vec<Node>,
    },
    Stack {
        children: Vec<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        header: Option<Header>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Titlebar {
    #[default]
    System,
    #[serde(rename = "none")]
    Layout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClonedFrom {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloned_from: Option<ClonedFrom>,
    #[serde(flatten)]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Line>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<Padding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<FocusLook>,
    #[serde(flatten)]
    pub look: Look,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divider: Option<Line>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resizable: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub floating: Vec<Float>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titlebar: Option<Titlebar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traffic_lights: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_menu: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub window_drag: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    Row,
    Column,
}
