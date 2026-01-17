use crate::domain::entities::CachedUser;

#[derive(Debug, Clone, Default)]
pub struct AutocompleteState {
    pub active: bool,
    pub query: String,
    pub trigger_index: usize,
    pub results: Vec<CachedUser>,
    pub selected_index: usize,
}

impl AutocompleteState {
    #[must_use]
    pub fn selected_user(&self) -> Option<&CachedUser> {
        self.results.get(self.selected_index)
    }
}

pub struct AutocompleteService {
    state: AutocompleteState,
}

impl AutocompleteService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AutocompleteState::default(),
        }
    }

    #[must_use]
    pub fn state(&self) -> &AutocompleteState {
        &self.state
    }

    pub fn reset(&mut self) {
        self.state.active = false;
        self.state.query.clear();
        self.state.results.clear();
        self.state.selected_index = 0;
    }

    pub fn process_input(&mut self, text: &str, cursor_idx: usize) -> bool {
        if cursor_idx == 0 {
            if self.state.active {
                self.reset();
                return true;
            }
            return false;
        }

        let slice_up_to_cursor = &text[..cursor_idx];

        if let Some(last_at_index) = slice_up_to_cursor.rfind('@') {
            let valid_trigger = if last_at_index == 0 {
                true
            } else {
                let char_before = slice_up_to_cursor[..last_at_index]
                    .chars()
                    .last()
                    .unwrap_or(' ');
                char_before.is_whitespace()
            };

            if valid_trigger {
                let query = &slice_up_to_cursor[last_at_index + 1..];

                if query.contains('\n') {
                    if self.state.active {
                        self.reset();
                        return true;
                    }
                    return false;
                }

                let new_query = query.to_string();
                if !self.state.active || self.state.query != new_query {
                    self.state.active = true;
                    self.state.query = new_query;
                    self.state.trigger_index = last_at_index;
                    return true;
                }
                return false;
            }
        }

        if self.state.active {
            self.reset();
            return true;
        }
        false
    }

    pub fn update_results(&mut self, candidates: Vec<CachedUser>) {
        let lower_query = self.state.query.to_lowercase();
        self.state.results = candidates
            .into_iter()
            .filter(|user| {
                user.username().to_lowercase().contains(&lower_query)
                    || user.display_name().to_lowercase().contains(&lower_query)
            })
            .collect();

        if self.state.selected_index >= self.state.results.len() {
            self.state.selected_index = 0;
        }
    }

    pub fn select_next(&mut self) {
        if !self.state.results.is_empty() {
            if self.state.selected_index >= self.state.results.len() - 1 {
                self.state.selected_index = 0;
            } else {
                self.state.selected_index += 1;
            }
        }
    }

    pub fn select_previous(&mut self) {
        if !self.state.results.is_empty() {
            if self.state.selected_index == 0 {
                self.state.selected_index = self.state.results.len() - 1;
            } else {
                self.state.selected_index -= 1;
            }
        }
    }
}

impl Default for AutocompleteService {
    fn default() -> Self {
        Self::new()
    }
}
