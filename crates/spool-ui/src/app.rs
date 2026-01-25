use anyhow::Result;
use spool::context::SpoolContext;
use spool::event::Event;
use spool::state::{load_or_materialize_state, Stream, Task, TaskStatus};
use spool::writer::{self, CreateTaskParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    TaskList,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    Open,
    Complete,
    All,
}

impl StatusFilter {
    pub fn label(&self) -> &'static str {
        match self {
            StatusFilter::Open => "Open",
            StatusFilter::Complete => "Complete",
            StatusFilter::All => "All",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            StatusFilter::Open => StatusFilter::Complete,
            StatusFilter::Complete => StatusFilter::All,
            StatusFilter::All => StatusFilter::Open,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Priority,
    Created,
    Title,
}

impl SortBy {
    pub fn label(&self) -> &'static str {
        match self {
            SortBy::Priority => "Priority",
            SortBy::Created => "Created",
            SortBy::Title => "Title",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SortBy::Priority => SortBy::Created,
            SortBy::Created => SortBy::Title,
            SortBy::Title => SortBy::Priority,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    NewTask,
}

pub struct App {
    // Input state
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub message: Option<String>,
    pub detail_scroll: u16,
    pub detail_content_height: u16, // set by UI during render
    pub detail_visible_height: u16, // set by UI during render
    pub tasks: Vec<Task>,
    pub streams: std::collections::HashMap<String, Stream>,
    pub stream_ids: Vec<String>, // sorted list of stream IDs for cycling
    pub stream_filter: Option<String>, // None = all streams
    pub selected: usize,
    pub focus: Focus,
    pub show_detail: bool,
    pub show_events: bool,
    pub status_filter: StatusFilter,
    pub sort_by: SortBy,
    pub search_query: String,
    pub search_mode: bool,
    pub task_events: Vec<Event>,
    ctx: SpoolContext,
}

impl App {
    pub fn new() -> Result<Self> {
        let ctx = SpoolContext::discover()?;
        let state = load_or_materialize_state(&ctx)?;

        let streams = state.streams.clone();
        let mut stream_ids: Vec<String> = streams.keys().cloned().collect();
        stream_ids.sort_by(|a, b| {
            let name_a = streams.get(a).map(|s| s.name.as_str()).unwrap_or(a);
            let name_b = streams.get(b).map(|s| s.name.as_str()).unwrap_or(b);
            name_a.to_lowercase().cmp(&name_b.to_lowercase())
        });

        let mut tasks: Vec<Task> = state
            .tasks
            .into_values()
            .filter(|t| t.status == TaskStatus::Open)
            .collect();

        tasks.sort_by(|a, b| {
            let pa = a.priority.as_deref().unwrap_or("p3");
            let pb = b.priority.as_deref().unwrap_or("p3");
            pa.cmp(pb).then_with(|| a.created.cmp(&b.created))
        });

        Ok(Self {
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            message: None,
            detail_scroll: 0,
            detail_content_height: 0,
            detail_visible_height: 0,
            tasks,
            streams,
            stream_ids,
            stream_filter: None,
            selected: 0,
            focus: Focus::TaskList,
            show_detail: false,
            show_events: false,
            status_filter: StatusFilter::Open,
            sort_by: SortBy::Priority,
            search_query: String::new(),
            search_mode: false,
            task_events: Vec::new(),
            ctx,
        })
    }

    pub fn reload_tasks(&mut self) -> Result<()> {
        let state = load_or_materialize_state(&self.ctx)?;
        self.streams = state.streams.clone();

        // Update stream_ids list
        self.stream_ids = self.streams.keys().cloned().collect();
        self.stream_ids.sort_by(|a, b| {
            let name_a = self.streams.get(a).map(|s| s.name.as_str()).unwrap_or(a);
            let name_b = self.streams.get(b).map(|s| s.name.as_str()).unwrap_or(b);
            name_a.to_lowercase().cmp(&name_b.to_lowercase())
        });

        // Validate stream_filter still exists
        if let Some(ref filter) = self.stream_filter {
            if !self.streams.contains_key(filter) {
                self.stream_filter = None;
            }
        }

        let query = self.search_query.to_lowercase();
        let stream_filter = self.stream_filter.clone();
        let mut tasks: Vec<Task> = state
            .tasks
            .into_values()
            .filter(|t| match self.status_filter {
                StatusFilter::Open => t.status == TaskStatus::Open,
                StatusFilter::Complete => t.status == TaskStatus::Complete,
                StatusFilter::All => true,
            })
            .filter(|t| match &stream_filter {
                None => true,
                Some(stream_id) => t.stream.as_ref() == Some(stream_id),
            })
            .filter(|t| {
                if query.is_empty() {
                    true
                } else {
                    t.title.to_lowercase().contains(&query)
                        || t.description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(&query))
                            .unwrap_or(false)
                        || t.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
                }
            })
            .collect();

        self.sort_tasks(&mut tasks);
        self.tasks = tasks;

        if self.selected >= self.tasks.len() && !self.tasks.is_empty() {
            self.selected = self.tasks.len() - 1;
        }
        if self.tasks.is_empty() {
            self.selected = 0;
        }

        Ok(())
    }

    fn sort_tasks(&self, tasks: &mut [Task]) {
        match self.sort_by {
            SortBy::Priority => {
                tasks.sort_by(|a, b| {
                    let pa = a.priority.as_deref().unwrap_or("p3");
                    let pb = b.priority.as_deref().unwrap_or("p3");
                    pa.cmp(pb).then_with(|| a.created.cmp(&b.created))
                });
            }
            SortBy::Created => {
                tasks.sort_by(|a, b| b.created.cmp(&a.created));
            }
            SortBy::Title => {
                tasks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            }
        }
    }

    pub fn load_task_events(&mut self) -> Result<()> {
        if let Some(task) = self.selected_task() {
            let task_id = task.id.clone();
            let events_by_task = spool::archive::collect_all_events(&self.ctx)?;
            self.task_events = events_by_task
                .into_values()
                .flatten()
                .filter(|e| e.id == task_id)
                .collect();
        }
        Ok(())
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.tasks.get(self.selected)
    }

    pub fn get_stream(&self, id: &str) -> Option<&Stream> {
        self.streams.get(id)
    }

    pub fn next_task(&mut self) {
        if !self.tasks.is_empty() {
            self.selected = (self.selected + 1).min(self.tasks.len() - 1);
            self.detail_scroll = 0;
        }
    }

    pub fn previous_task(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.detail_scroll = 0;
    }

    pub fn scroll_detail_down(&mut self) {
        let max_scroll = self
            .detail_content_height
            .saturating_sub(self.detail_visible_height);
        if self.detail_scroll < max_scroll {
            self.detail_scroll = self.detail_scroll.saturating_add(1);
        }
    }

    pub fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    pub fn first_task(&mut self) {
        self.selected = 0;
    }

    pub fn last_task(&mut self) {
        if !self.tasks.is_empty() {
            self.selected = self.tasks.len() - 1;
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::TaskList => Focus::Detail,
            Focus::Detail => Focus::TaskList,
        };
    }

    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
    }

    pub fn toggle_events(&mut self) {
        self.show_events = !self.show_events;
        if self.show_events {
            let _ = self.load_task_events();
        }
    }

    pub fn cycle_status_filter(&mut self) {
        self.status_filter = self.status_filter.next();
        let _ = self.reload_tasks();
    }

    pub fn cycle_sort(&mut self) {
        self.sort_by = self.sort_by.next();
        let _ = self.reload_tasks();
    }

    pub fn toggle_search(&mut self) {
        self.search_mode = !self.search_mode;
        if !self.search_mode && !self.search_query.is_empty() {
            let _ = self.reload_tasks();
        }
    }

    pub fn search_input(&mut self, c: char) {
        self.search_query.push(c);
        let _ = self.reload_tasks();
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        let _ = self.reload_tasks();
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        let _ = self.reload_tasks();
    }

    pub fn cycle_stream_filter(&mut self) {
        if self.stream_ids.is_empty() {
            return;
        }

        self.stream_filter = match &self.stream_filter {
            None => Some(self.stream_ids[0].clone()),
            Some(current) => {
                let idx = self.stream_ids.iter().position(|id| id == current);
                match idx {
                    Some(i) if i + 1 < self.stream_ids.len() => {
                        Some(self.stream_ids[i + 1].clone())
                    }
                    _ => None, // wrap back to "All"
                }
            }
        };
        let _ = self.reload_tasks();
    }

    pub fn stream_filter_label(&self) -> String {
        match &self.stream_filter {
            None => "All".to_string(),
            Some(id) => self
                .streams
                .get(id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| id.clone()),
        }
    }

    // Task editing methods

    pub fn complete_selected_task(&mut self) {
        if let Some(task) = self.selected_task() {
            if task.status == TaskStatus::Complete {
                self.message = Some("Task already complete".to_string());
                return;
            }
            let id = task.id.clone();
            let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
            let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

            match writer::complete_task(&self.ctx, &id, None, &by, &branch) {
                Ok(()) => {
                    self.message = Some(format!("Completed: {}", id));
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }
    }

    pub fn reopen_selected_task(&mut self) {
        if let Some(task) = self.selected_task() {
            if task.status == TaskStatus::Open {
                self.message = Some("Task already open".to_string());
                return;
            }
            let id = task.id.clone();
            let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
            let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

            match writer::reopen_task(&self.ctx, &id, &by, &branch) {
                Ok(()) => {
                    self.message = Some(format!("Reopened: {}", id));
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }
    }

    pub fn start_new_task(&mut self) {
        self.input_mode = InputMode::NewTask;
        self.input_buffer.clear();
        self.message = None;
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn submit_new_task(&mut self) {
        if self.input_buffer.trim().is_empty() {
            self.message = Some("Title cannot be empty".to_string());
            self.input_mode = InputMode::Normal;
            return;
        }

        let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
        let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

        let params = CreateTaskParams {
            title: self.input_buffer.trim(),
            stream: self.stream_filter.as_deref(),
            ..Default::default()
        };

        match writer::create_task(&self.ctx, params, &by, &branch) {
            Ok(id) => {
                self.message = Some(format!("Created: {}", id));
                let _ = self.reload_tasks();
            }
            Err(e) => {
                self.message = Some(format!("Error: {}", e));
            }
        }

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn input_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    pub fn input_backspace(&mut self) {
        self.input_buffer.pop();
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }
}
