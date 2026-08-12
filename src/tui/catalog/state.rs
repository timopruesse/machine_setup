use std::collections::BTreeSet;

use super::model::{CatalogItem, CatalogMode};

#[derive(Debug, Clone)]
pub struct CatalogState {
    pub items: Vec<CatalogItem>,
    pub mode: CatalogMode,
    pub selected: usize,
    pub checked: BTreeSet<usize>,
    pub search_mode: bool,
    pub search_query: String,
    pub filtered_indices: Vec<usize>,
}

impl CatalogState {
    pub fn new(items: Vec<CatalogItem>, mode: CatalogMode) -> Self {
        let filtered_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            items,
            mode,
            selected: 0,
            checked: BTreeSet::new(),
            search_mode: false,
            search_query: String::new(),
            filtered_indices,
        }
    }

    pub fn filter_active(&self) -> bool {
        self.search_mode || !self.search_query.is_empty()
    }

    pub fn refresh_filter(&mut self) {
        let q = self.search_query.to_lowercase();
        if q.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            self.filtered_indices = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    item.title.to_lowercase().contains(&q) || item.id.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if !self.filtered_indices.is_empty() && !self.filtered_indices.contains(&self.selected) {
            self.selected = self.filtered_indices[0];
        }
    }
}
