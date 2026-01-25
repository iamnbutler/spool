use anyhow::Result;
use spool::context::SpoolContext;
use spool::event::Event;
use spool::state::{load_or_materialize_state, Stream, Task, TaskStatus};

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

pub struct App {
    pub tasks: Vec<Task>,
    pub streams: std::collections::HashMap<String, Stream>,
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
            tasks,
            streams,
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

        let query = self.search_query.to_lowercase();
        let mut tasks: Vec<Task> = state
            .tasks
            .into_values()
            .filter(|t| match self.status_filter {
                StatusFilter::Open => t.status == TaskStatus::Open,
                StatusFilter::Complete => t.status == TaskStatus::Complete,
                StatusFilter::All => true,
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
        }
    }

    pub fn previous_task(&mut self) {
        self.selected = self.selected.saturating_sub(1);
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
}
