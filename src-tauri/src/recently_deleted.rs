use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeletedItem {
    pub id: String,
    pub item_type: DeletedItemType,
    pub title: String,
    pub deleted_at_ms: i64,
    pub original_source: DeletedItemSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DeletedItemType {
    Credential,
    Note,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum DeletedItemSource {
    Vault,
    Notes,
}
