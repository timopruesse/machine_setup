/// Installation / visibility status for a catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogStatus {
    Installed,
    NotInstalled,
    SkippedOs,
    Neutral,
}

/// One titled block in an item's detail panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailSection {
    pub title: String,
    pub lines: Vec<String>,
}

/// A single row in the catalog list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogItem {
    pub id: String,
    pub title: String,
    pub status: CatalogStatus,
    pub os_label: String,
    pub installed_at: String,
    pub updated_at: String,
    pub badges: Vec<String>,
    pub detail: Vec<DetailSection>,
}

/// How the catalog viewer behaves after launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogMode {
    Browse,
    Select,
}
