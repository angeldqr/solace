use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::terminals::{Layout, PaneId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpaceId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TabId(pub u32);

/// A project folder with its own tabs of terminals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub root: PathBuf,
    pub tabs: Vec<Tab>,
    pub active_tab: Option<TabId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: TabId,
    pub layout: Layout,
    pub focused: PaneId,
}
