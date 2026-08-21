#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Structure,
    BrowseData,
    ExecuteSql,
}

impl Tab {
    pub fn all() -> [Tab; 3] {
        [Tab::Structure, Tab::BrowseData, Tab::ExecuteSql]
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Structure => "Structure",
            Tab::BrowseData => "Browse",
            Tab::ExecuteSql => "SQL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn sql(self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            SortDirection::Asc => "▲",
            SortDirection::Desc => "▼",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GridState {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub filters: Vec<String>,
    pub widths: Vec<f32>,
    pub selected_cell: Option<(usize, usize)>,
    pub sort: Option<(usize, SortDirection)>,
    pub page: u32,
    pub total_records: u64,
    pub page_size: u32,
    pub all_results: bool,
}

impl Default for GridState {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
            filters: Vec::new(),
            widths: Vec::new(),
            selected_cell: None,
            sort: None,
            page: 0,
            total_records: 0,
            page_size: 100,
            all_results: false,
        }
    }
}

impl GridState {
    pub fn calculate_initial_widths(&mut self) {
        if self.headers.is_empty() {
            return;
        }
        let mut new_widths = Vec::new();
        for (i, name) in self.headers.iter().enumerate() {
            let mut max_chars = name.len() + 6;
            for row in self.rows.iter().take(50) {
                if let Some(val) = row.get(i) {
                    max_chars = max_chars.max(val.len());
                }
            }
            let pixel_width = (max_chars as f32 * 9.5 + 20.0).clamp(80.0, 600.0);
            new_widths.push(pixel_width);
        }
        self.widths = new_widths;
    }

    pub fn ensure_filters(&mut self) {
        let count = self.headers.len();
        if self.filters.len() != count {
            self.filters = vec![String::new(); count];
        }
    }

    pub fn apply_result(&mut self, headers: Vec<String>, rows: Vec<Vec<String>>) {
        self.headers = headers;
        self.rows = rows;
        self.calculate_initial_widths();
        self.ensure_filters();
    }

    pub fn total_pages(&self) -> u32 {
        if self.all_results || self.page_size == 0 {
            return 1;
        }
        let pages = (self.total_records as f32 / self.page_size as f32).ceil() as u32;
        pages.max(1)
    }

    pub fn limit_offset(&self) -> (Option<u32>, Option<u32>) {
        if self.all_results {
            (None, None)
        } else {
            (Some(self.page_size), Some(self.page * self.page_size))
        }
    }

    pub fn cell(&self, row: usize, col: usize) -> Option<&str> {
        self.rows.get(row).and_then(|r| r.get(col)).map(|s| s.as_str())
    }

    pub fn row_display(&self, row: usize) -> String {
        self.rows
            .get(row)
            .map(|r| r.join(" | "))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_state_default() {
        let state = GridState::default();
        assert!(state.rows.is_empty());
        assert_eq!(state.page, 0);
        assert_eq!(state.page_size, 100);
        assert_eq!(state.total_records, 0);
    }

    #[test]
    fn test_calculate_widths() {
        let mut state = GridState::default();
        state.headers = vec!["id".to_string(), "name".to_string()];
        state.rows = vec![vec![
            "1".to_string(),
            "Very long name that should expand the column".to_string(),
        ]];
        state.calculate_initial_widths();
        assert_eq!(state.widths.len(), 2);
        assert!(state.widths[1] > state.widths[0]);
    }
}
