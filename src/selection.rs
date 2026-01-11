//! Selection Handler
//!
//! Manages selection lists and pagination for the voice assistant.
//! Handles user selection from multi-result searches with voice commands.

use crate::players::SearchResult;
use tracing::{debug, info};

/// Number of items shown per page (matches Python)
const ITEMS_PER_PAGE: usize = 5;

/// Result of a selection command
#[derive(Debug, Clone)]
pub enum SelectionResult {
    /// User selected an item
    Selected(SearchResult, usize),
    /// User wants next page
    NextPage,
    /// User wants previous page
    PreviousPage,
    /// User cancelled selection
    Cancelled,
    /// Need to speak options (no action yet)
    SpeakOptions,
    /// Input not recognized
    NotRecognized,
}

/// State of the selection handler
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionState {
    /// No active selection
    Inactive,
    /// Waiting for user to select
    Active,
}

/// Handles selection lists, pagination, and user selection commands.
#[derive(Debug)]
pub struct SelectionHandler {
    /// Items to select from
    items: Vec<SearchResult>,
    /// Current page (0-indexed)
    page: usize,
    /// Field type being selected
    field: String,
    /// Custom title for selection
    title: String,
    /// Current state
    state: SelectionState,
}

impl Default for SelectionHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionHandler {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            page: 0,
            field: "result".to_string(),
            title: "Select an item".to_string(),
            state: SelectionState::Inactive,
        }
    }

    /// Set items for selection and activate
    pub fn set_items(&mut self, items: Vec<SearchResult>, field: &str) {
        info!("📋 Selection started: {} items for {}", items.len(), field);
        self.items = items;
        self.page = 0;
        self.field = field.to_string();
        self.state = SelectionState::Active;
    }

    /// Check if selection is active
    pub fn is_active(&self) -> bool {
        self.state == SelectionState::Active
    }

    /// Get current state
    pub fn state(&self) -> &SelectionState {
        &self.state
    }

    /// Get the items
    pub fn items(&self) -> &[SearchResult] {
        &self.items
    }

    /// Get current page
    pub fn page(&self) -> usize {
        self.page
    }

    /// Total pages
    pub fn total_pages(&self) -> usize {
        if self.items.is_empty() {
            0
        } else {
            (self.items.len() - 1) / ITEMS_PER_PAGE + 1
        }
    }

    /// Get items for current page
    pub fn current_page_items(&self) -> &[SearchResult] {
        let start = self.page * ITEMS_PER_PAGE;
        let end = std::cmp::min(start + ITEMS_PER_PAGE, self.items.len());
        &self.items[start..end]
    }

    /// Build spoken message for current page
    pub fn speak_options_text(&self) -> String {
        if self.items.is_empty() {
            return "No items to select from.".to_string();
        }

        let start_idx = self.page * ITEMS_PER_PAGE;
        let current_items = self.current_page_items();

        let mut spoken_items: Vec<String> = Vec::new();
        for (i, item) in current_items.iter().enumerate() {
            spoken_items.push(format!("{}. {}", start_idx + i + 1, item.display));
        }

        let mut msg = format!("Found {} matches. ", self.items.len());

        if self.total_pages() > 1 {
            msg = format!("Page {}. ", self.page + 1) + &msg;
        }

        msg += &spoken_items.join(", ");

        let end_idx = start_idx + current_items.len();
        if end_idx < self.items.len() {
            msg += ". Say 'next' for more.";
        }

        msg
    }

    /// Handle a selection command
    ///
    /// Returns the result of processing the command
    pub fn handle_command(&mut self, text: &str) -> SelectionResult {
        if !self.is_active() {
            return SelectionResult::NotRecognized;
        }

        let text = text.to_lowercase();
        let text = text.trim();

        // Pagination commands
        if text.contains("next") || text.contains("more") {
            let max_page = self.total_pages().saturating_sub(1);
            if self.page < max_page {
                self.page += 1;
                debug!("Selection: next page -> {}", self.page + 1);
                return SelectionResult::NextPage;
            }
            return SelectionResult::SpeakOptions; // Already on last page
        }

        if text.contains("previous") || text.contains("back") {
            if self.page > 0 {
                self.page -= 1;
                debug!("Selection: previous page -> {}", self.page + 1);
                return SelectionResult::PreviousPage;
            }
            return SelectionResult::SpeakOptions; // Already on first page
        }

        if text.contains("cancel")
            || text.contains("stop")
            || text.contains("quit")
            || text.contains("exit")
        {
            debug!("Selection: cancelled");
            self.clear();
            return SelectionResult::Cancelled;
        }

        // Strip common prefixes
        let mut clean_text = text.to_string();
        for prefix in &[
            "number ",
            "play number ",
            "option ",
            "play option ",
            "choice ",
            "play ",
        ] {
            if clean_text.starts_with(prefix) {
                clean_text = clean_text[prefix.len()..].to_string();
                break;
            }
        }

        // Try to parse a number
        if let Some(selection_index) = parse_number(&clean_text) {
            // Adjust for 0-based index (user says "1", we want index 0)
            let idx = selection_index.saturating_sub(1);

            if idx < self.items.len() {
                let selected = self.items[idx].clone();
                info!("📌 Selected: {} (index {})", selected.display, idx);
                self.clear();
                return SelectionResult::Selected(selected, idx);
            }
        }

        SelectionResult::NotRecognized
    }

    /// Clear selection state
    pub fn clear(&mut self) {
        self.items.clear();
        self.page = 0;
        self.field = "result".to_string();
        self.title = "Select an item".to_string();
        self.state = SelectionState::Inactive;
    }

    /// Reset for new selection
    pub fn reset(&mut self) {
        self.clear();
    }

    /// Set title
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.state = SelectionState::Active;
    }

    /// Set results
    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.items = results;
        self.state = SelectionState::Active;
    }

    /// Set page
    pub fn set_page(&mut self, page: usize) {
        self.page = page;
    }

    /// Handle a raw input string and return the selected index if successful
    pub fn handle_input(&mut self, text: &str) -> Option<usize> {
        match self.handle_command(text) {
            SelectionResult::Selected(_, idx) => Some(idx),
            _ => None,
        }
    }

    /// Get title
    pub fn title(&self) -> &str {
        &self.title
    }
}

/// Parse a number from text (1-99)
fn parse_number(text: &str) -> Option<usize> {
    let text = text.trim().to_lowercase();

    // Try direct digit parse
    if let Ok(n) = text.parse::<usize>() {
        if (1..=99).contains(&n) {
            return Some(n);
        }
    }

    // Word mappings
    let word_map = [
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        ("twenty", 20),
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
    ];

    for (word, num) in word_map {
        if text == word || text.starts_with(&format!("{} ", word)) {
            return Some(num);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::SearchResultType;

    fn make_result(display: &str, value: &str) -> SearchResult {
        SearchResult {
            display: display.to_string(),
            value: value.to_string(),
            result_type: SearchResultType::Artist,
            score: 0.9,
        }
    }

    #[test]
    fn test_selection_flow() {
        let mut handler = SelectionHandler::new();
        assert!(!handler.is_active());

        let items = vec![
            make_result("Artist: Beethoven", "beethoven"),
            make_result("Artist: Bach", "bach"),
            make_result("Album: Symphony No. 5", "sym5"),
        ];

        handler.set_items(items, "search");
        assert!(handler.is_active());
        assert_eq!(handler.items().len(), 3);

        // Test number selection
        let result = handler.handle_command("1");
        match result {
            SelectionResult::Selected(item, idx) => {
                assert_eq!(item.value, "beethoven");
                assert_eq!(idx, 0);
            }
            _ => panic!("Expected Selected"),
        }

        assert!(!handler.is_active());
    }

    #[test]
    fn test_pagination() {
        let mut handler = SelectionHandler::new();
        let items: Vec<SearchResult> = (1..=12)
            .map(|i| make_result(&format!("Item {}", i), &format!("item{}", i)))
            .collect();

        handler.set_items(items, "test");
        assert_eq!(handler.total_pages(), 3);
        assert_eq!(handler.current_page_items().len(), 5);

        handler.handle_command("next");
        assert_eq!(handler.page(), 1);

        handler.handle_command("previous");
        assert_eq!(handler.page(), 0);
    }

    #[test]
    fn test_cancel() {
        let mut handler = SelectionHandler::new();
        handler.set_items(vec![make_result("Test", "test")], "test");
        assert!(handler.is_active());

        let result = handler.handle_command("cancel");
        assert!(matches!(result, SelectionResult::Cancelled));
        assert!(!handler.is_active());
    }
}
