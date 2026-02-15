use anyhow::Result;
use ratatui::widgets::ListState;
use spool::context::SpoolContext;
use spool::event::Event;
use spool::state::{load_or_materialize_state, Stream, Task, TaskStatus};
use spool::writer::{self, CreateTaskParams};
use spool::{archive, init, rebuild, validation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tasks,
    Streams,
    History,
}

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
    NewStream,
    EditTaskTitle,
    EditTaskPriority,
    EditStreamName,
    AssignTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditField {
    Title,
    Priority,
}

impl EditField {
    pub fn all() -> &'static [EditField] {
        &[EditField::Title, EditField::Priority]
    }

    pub fn label(&self) -> &'static str {
        match self {
            EditField::Title => "Title",
            EditField::Priority => "Priority",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Rebuild,
    Validate,
    Archive,
}

impl Command {
    pub fn label(&self) -> &'static str {
        match self {
            Command::Rebuild => "Rebuild cache",
            Command::Validate => "Validate events",
            Command::Archive => "Archive old tasks",
        }
    }

    pub fn all() -> &'static [Command] {
        &[Command::Rebuild, Command::Validate, Command::Archive]
    }
}

pub struct App {
    // Input state
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub message: Option<String>,
    pub pending_quit: bool,
    pub show_help: bool,
    pub show_command_palette: bool,
    pub command_selected: usize,
    pub show_edit_menu: bool,
    pub edit_field_selected: usize,
    pub editing_task_id: Option<String>,
    pub editing_stream_id: Option<String>,
    pub pending_delete_stream: Option<String>, // stream ID pending deletion
    pub detail_scroll: u16,
    pub detail_content_height: u16, // set by UI during render
    pub detail_visible_height: u16, // set by UI during render
    pub tasks: Vec<Task>,
    pub streams: std::collections::HashMap<String, Stream>,
    pub stream_ids: Vec<String>, // sorted list of stream IDs for cycling
    pub stream_filter: Option<String>, // None = all streams
    pub selected: usize,
    pub task_list_state: ListState,
    pub focus: Focus,
    pub show_detail: bool,
    pub status_filter: StatusFilter,
    pub sort_by: SortBy,
    pub search_query: String,
    pub search_mode: bool,
    pub task_events: Vec<Event>,
    // View state
    pub view: View,
    // Streams view state
    pub streams_selected: usize,
    pub streams_list_state: ListState,
    // History view state
    pub history_events: Vec<Event>,
    pub history_selected: usize,
    pub history_list_state: ListState,
    pub history_scroll_x: u16,
    pub history_show_detail: bool,
    pub history_detail_scroll: u16,
    pub all_tasks: std::collections::HashMap<String, Task>, // for name lookups
    ctx: SpoolContext,
}

impl App {
    pub fn new() -> Result<Self> {
        // Try to discover existing spool, or auto-init if not found
        let ctx = match SpoolContext::discover() {
            Ok(ctx) => ctx,
            Err(_) => {
                // Auto-initialize spool directory
                init()?;
                SpoolContext::discover()?
            }
        };
        let state = load_or_materialize_state(&ctx)?;

        let streams = state.streams.clone();
        let mut stream_ids: Vec<String> = streams.keys().cloned().collect();
        stream_ids.sort_by(|a, b| {
            let name_a = streams.get(a).map(|s| s.name.as_str()).unwrap_or(a);
            let name_b = streams.get(b).map(|s| s.name.as_str()).unwrap_or(b);
            name_a.to_lowercase().cmp(&name_b.to_lowercase())
        });

        let all_tasks = state.tasks.clone();

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
            pending_quit: false,
            show_help: false,
            show_command_palette: false,
            command_selected: 0,
            show_edit_menu: false,
            edit_field_selected: 0,
            editing_task_id: None,
            editing_stream_id: None,
            pending_delete_stream: None,
            detail_scroll: 0,
            detail_content_height: 0,
            detail_visible_height: 0,
            tasks,
            streams,
            stream_ids,
            stream_filter: None,
            selected: 0,
            task_list_state: ListState::default().with_selected(Some(0)),
            focus: Focus::TaskList,
            show_detail: false,
            status_filter: StatusFilter::Open,
            sort_by: SortBy::Priority,
            search_query: String::new(),
            search_mode: false,
            task_events: Vec::new(),
            view: View::Tasks,
            streams_selected: 0,
            streams_list_state: ListState::default().with_selected(Some(0)),
            history_events: Vec::new(),
            history_selected: 0,
            history_list_state: ListState::default().with_selected(Some(0)),
            history_scroll_x: 0,
            history_show_detail: false,
            history_detail_scroll: 0,
            all_tasks,
            ctx,
        })
    }

    /// Returns the path to the events directory for file watching
    pub fn events_dir(&self) -> &std::path::Path {
        &self.ctx.events_dir
    }

    pub fn reload_tasks(&mut self) -> Result<()> {
        let state = load_or_materialize_state(&self.ctx)?;
        self.streams = state.streams.clone();
        self.all_tasks = state.tasks.clone();

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
        self.task_list_state.select(Some(self.selected));

        // Also sync streams list state
        if self.streams_selected >= self.stream_ids.len() && !self.stream_ids.is_empty() {
            self.streams_selected = self.stream_ids.len() - 1;
        }
        if self.stream_ids.is_empty() {
            self.streams_selected = 0;
        }
        self.streams_list_state.select(Some(self.streams_selected));

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
            self.task_list_state.select(Some(self.selected));
            self.detail_scroll = 0;
            if self.show_detail {
                let _ = self.load_task_events();
            }
        }
    }

    pub fn previous_task(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.task_list_state.select(Some(self.selected));
        self.detail_scroll = 0;
        if self.show_detail {
            let _ = self.load_task_events();
        }
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
        self.task_list_state.select(Some(0));
    }

    pub fn last_task(&mut self) {
        if !self.tasks.is_empty() {
            self.selected = self.tasks.len() - 1;
            self.task_list_state.select(Some(self.selected));
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
        if self.show_detail {
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

    // Task edit menu
    pub fn show_task_edit_menu(&mut self) {
        if let Some(task) = self.selected_task() {
            self.editing_task_id = Some(task.id.clone());
            self.show_edit_menu = true;
            self.edit_field_selected = 0;
        }
    }

    pub fn close_edit_menu(&mut self) {
        self.show_edit_menu = false;
        self.editing_task_id = None;
        self.editing_stream_id = None;
        self.edit_field_selected = 0;
    }

    pub fn edit_menu_next(&mut self) {
        let fields = EditField::all();
        if !fields.is_empty() {
            self.edit_field_selected = (self.edit_field_selected + 1).min(fields.len() - 1);
        }
    }

    pub fn edit_menu_previous(&mut self) {
        self.edit_field_selected = self.edit_field_selected.saturating_sub(1);
    }

    pub fn start_editing_selected_field(&mut self) {
        let fields = EditField::all();
        if let Some(field) = fields.get(self.edit_field_selected) {
            if let Some(task_id) = &self.editing_task_id {
                // Get current value
                let current_value = if let Some(task) = self.all_tasks.get(task_id) {
                    match field {
                        EditField::Title => task.title.clone(),
                        EditField::Priority => task.priority.clone().unwrap_or_default(),
                    }
                } else {
                    String::new()
                };

                self.input_buffer = current_value;
                self.show_edit_menu = false;
                self.input_mode = match field {
                    EditField::Title => InputMode::EditTaskTitle,
                    EditField::Priority => InputMode::EditTaskPriority,
                };
            }
        }
    }

    pub fn submit_task_edit(&mut self) {
        let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
        let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

        if let Some(task_id) = self.editing_task_id.take() {
            let result = match self.input_mode {
                InputMode::EditTaskTitle => {
                    if self.input_buffer.trim().is_empty() {
                        self.message = Some("Title cannot be empty".to_string());
                        self.input_mode = InputMode::Normal;
                        return;
                    }
                    writer::update_task(
                        &self.ctx,
                        &task_id,
                        Some(self.input_buffer.trim()),
                        None,
                        None,
                        &by,
                        &branch,
                    )
                }
                InputMode::EditTaskPriority => {
                    let priority = if self.input_buffer.trim().is_empty() {
                        None
                    } else {
                        Some(self.input_buffer.trim())
                    };
                    writer::update_task(&self.ctx, &task_id, None, None, priority, &by, &branch)
                }
                _ => Ok(()),
            };

            match result {
                Ok(()) => {
                    self.message = Some("Task updated".to_string());
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    // Task assignment
    pub fn claim_selected_task(&mut self) {
        if let Some(task) = self.selected_task() {
            let id = task.id.clone();
            let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
            let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

            match writer::assign_task(&self.ctx, &id, Some(&by), &by, &branch) {
                Ok(()) => {
                    self.message = Some(format!("Claimed: {}", id));
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }
    }

    pub fn free_selected_task(&mut self) {
        if let Some(task) = self.selected_task() {
            if task.assignee.is_none() {
                self.message = Some("Task not assigned".to_string());
                return;
            }
            let id = task.id.clone();
            let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
            let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

            match writer::assign_task(&self.ctx, &id, None, &by, &branch) {
                Ok(()) => {
                    self.message = Some(format!("Unassigned: {}", id));
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }
    }

    pub fn start_assign_task(&mut self) {
        if let Some(task) = self.selected_task() {
            let task_id = task.id.clone();
            let assignee = task.assignee.clone().unwrap_or_default();
            self.editing_task_id = Some(task_id);
            self.input_buffer = assignee;
            self.input_mode = InputMode::AssignTask;
        }
    }

    pub fn submit_assign_task(&mut self) {
        let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
        let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

        if let Some(task_id) = self.editing_task_id.take() {
            let assignee = if self.input_buffer.trim().is_empty() {
                None
            } else {
                Some(self.input_buffer.trim())
            };

            match writer::assign_task(&self.ctx, &task_id, assignee, &by, &branch) {
                Ok(()) => {
                    let msg = match assignee {
                        Some(a) => format!("Assigned to {}", a),
                        None => "Unassigned".to_string(),
                    };
                    self.message = Some(msg);
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
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
        self.pending_quit = false;
        self.pending_delete_stream = None;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_command_palette(&mut self) {
        self.show_command_palette = !self.show_command_palette;
        self.command_selected = 0;
    }

    pub fn command_next(&mut self) {
        let commands = Command::all();
        if !commands.is_empty() {
            self.command_selected = (self.command_selected + 1).min(commands.len() - 1);
        }
    }

    pub fn command_previous(&mut self) {
        self.command_selected = self.command_selected.saturating_sub(1);
    }

    pub fn execute_selected_command(&mut self) {
        let commands = Command::all();
        if let Some(cmd) = commands.get(self.command_selected) {
            self.show_command_palette = false;
            match cmd {
                Command::Rebuild => match rebuild(&self.ctx) {
                    Ok(()) => {
                        self.message = Some("Cache rebuilt successfully".to_string());
                        let _ = self.reload_tasks();
                    }
                    Err(e) => {
                        self.message = Some(format!("Rebuild failed: {}", e));
                    }
                },
                Command::Validate => match validation::validate(&self.ctx, false) {
                    Ok(result) => {
                        if result.errors.is_empty() && result.warnings.is_empty() {
                            self.message = Some("Validation passed".to_string());
                        } else {
                            self.message = Some(format!(
                                "Validation: {} errors, {} warnings",
                                result.errors.len(),
                                result.warnings.len()
                            ));
                        }
                    }
                    Err(e) => {
                        self.message = Some(format!("Validation failed: {}", e));
                    }
                },
                Command::Archive => match archive::archive_tasks(&self.ctx, 30, false) {
                    Ok(archived_ids) => {
                        if !archived_ids.is_empty() {
                            self.message = Some(format!("Archived {} tasks", archived_ids.len()));
                            let _ = self.reload_tasks();
                        } else {
                            self.message = Some("No tasks to archive".to_string());
                        }
                    }
                    Err(e) => {
                        self.message = Some(format!("Archive failed: {}", e));
                    }
                },
            }
        }
    }

    /// Returns true if should quit, false if showing confirmation
    pub fn request_quit(&mut self) -> bool {
        if self.pending_quit {
            true
        } else {
            self.pending_quit = true;
            self.message = Some("Press Esc again to quit".to_string());
            false
        }
    }

    // View navigation methods

    pub fn next_view(&mut self) {
        self.view = match self.view {
            View::Tasks => View::Streams,
            View::Streams => View::History,
            View::History => View::Tasks,
        };
        self.on_view_change();
    }

    pub fn previous_view(&mut self) {
        self.view = match self.view {
            View::Tasks => View::History,
            View::History => View::Streams,
            View::Streams => View::Tasks,
        };
        self.on_view_change();
    }

    fn on_view_change(&mut self) {
        match self.view {
            View::Tasks => {}
            View::Streams => {
                self.streams_selected = 0;
            }
            View::History => {
                let _ = self.load_history();
            }
        }
    }

    // Streams view methods

    pub fn toggle_streams_view(&mut self) {
        match self.view {
            View::Streams => {
                self.view = View::Tasks;
            }
            _ => {
                self.view = View::Streams;
                self.streams_selected = 0;
            }
        }
    }

    pub fn streams_next(&mut self) {
        if !self.stream_ids.is_empty() {
            self.streams_selected = (self.streams_selected + 1).min(self.stream_ids.len() - 1);
            self.streams_list_state.select(Some(self.streams_selected));
        }
    }

    pub fn streams_previous(&mut self) {
        self.streams_selected = self.streams_selected.saturating_sub(1);
        self.streams_list_state.select(Some(self.streams_selected));
    }

    pub fn streams_first(&mut self) {
        self.streams_selected = 0;
        self.streams_list_state.select(Some(0));
    }

    pub fn streams_last(&mut self) {
        if !self.stream_ids.is_empty() {
            self.streams_selected = self.stream_ids.len() - 1;
            self.streams_list_state.select(Some(self.streams_selected));
        }
    }

    pub fn select_current_stream(&mut self) {
        if let Some(stream_id) = self.stream_ids.get(self.streams_selected) {
            self.stream_filter = Some(stream_id.clone());
            self.view = View::Tasks;
            let _ = self.reload_tasks();
        }
    }

    pub fn start_new_stream(&mut self) {
        self.input_mode = InputMode::NewStream;
        self.input_buffer.clear();
        self.message = None;
    }

    pub fn submit_new_stream(&mut self) {
        if self.input_buffer.trim().is_empty() {
            self.message = Some("Name cannot be empty".to_string());
            self.input_mode = InputMode::Normal;
            return;
        }

        let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
        let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

        match writer::create_stream(&self.ctx, self.input_buffer.trim(), None, &by, &branch) {
            Ok(id) => {
                self.message = Some(format!("Created stream: {}", self.input_buffer.trim()));
                let _ = self.reload_tasks(); // This also reloads streams
                                             // Select the new stream
                if let Some(pos) = self.stream_ids.iter().position(|s| s == &id) {
                    self.streams_selected = pos;
                }
            }
            Err(e) => {
                self.message = Some(format!("Error: {}", e));
            }
        }

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn start_edit_stream(&mut self) {
        if let Some(stream_id) = self.stream_ids.get(self.streams_selected).cloned() {
            if let Some(stream) = self.streams.get(&stream_id) {
                self.editing_stream_id = Some(stream_id);
                self.input_buffer = stream.name.clone();
                self.input_mode = InputMode::EditStreamName;
            }
        }
    }

    pub fn submit_stream_edit(&mut self) {
        let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
        let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

        if let Some(stream_id) = self.editing_stream_id.take() {
            if self.input_buffer.trim().is_empty() {
                self.message = Some("Name cannot be empty".to_string());
                self.input_mode = InputMode::Normal;
                return;
            }

            match writer::update_stream(
                &self.ctx,
                &stream_id,
                Some(self.input_buffer.trim()),
                None,
                &by,
                &branch,
            ) {
                Ok(()) => {
                    self.message = Some("Stream updated".to_string());
                    let _ = self.reload_tasks();
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn request_delete_stream(&mut self) {
        if let Some(stream_id) = self.stream_ids.get(self.streams_selected).cloned() {
            // Check if stream has tasks
            let task_count = self
                .all_tasks
                .values()
                .filter(|t| t.stream.as_ref() == Some(&stream_id))
                .count();

            if task_count > 0 {
                self.message = Some(format!(
                    "Cannot delete: stream has {} task{}",
                    task_count,
                    if task_count == 1 { "" } else { "s" }
                ));
                return;
            }

            let stream_name = self
                .streams
                .get(&stream_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| stream_id.clone());

            self.pending_delete_stream = Some(stream_id);
            self.message = Some(format!(
                "Delete \"{}\"? Press d again to confirm",
                stream_name
            ));
        }
    }

    pub fn confirm_delete_stream(&mut self) {
        if let Some(stream_id) = self.pending_delete_stream.take() {
            let by = writer::get_current_user().unwrap_or_else(|_| "unknown".to_string());
            let branch = writer::get_current_branch().unwrap_or_else(|_| "main".to_string());

            match writer::delete_stream(&self.ctx, &stream_id, &by, &branch) {
                Ok(()) => {
                    self.message = Some("Stream deleted".to_string());
                    let _ = self.reload_tasks();
                    // Adjust selection if needed
                    if self.streams_selected >= self.stream_ids.len() && !self.stream_ids.is_empty()
                    {
                        self.streams_selected = self.stream_ids.len() - 1;
                    }
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
        }
    }

    // History view methods

    pub fn toggle_history_view(&mut self) {
        match self.view {
            View::Tasks | View::Streams => {
                self.view = View::History;
                let _ = self.load_history();
            }
            View::History => {
                self.view = View::Tasks;
            }
        }
    }

    pub fn load_history(&mut self) -> Result<()> {
        let events_by_task = spool::archive::collect_all_events(&self.ctx)?;
        let mut all_events: Vec<Event> = events_by_task.into_values().flatten().collect();
        // Sort by timestamp descending (most recent first)
        all_events.sort_by(|a, b| b.ts.cmp(&a.ts));
        self.history_events = all_events;
        self.history_selected = 0;
        self.history_list_state.select(Some(0));
        Ok(())
    }

    pub fn history_next(&mut self) {
        if !self.history_events.is_empty() {
            self.history_selected = (self.history_selected + 1).min(self.history_events.len() - 1);
            self.history_list_state.select(Some(self.history_selected));
        }
    }

    pub fn history_previous(&mut self) {
        self.history_selected = self.history_selected.saturating_sub(1);
        self.history_list_state.select(Some(self.history_selected));
    }

    pub fn history_first(&mut self) {
        self.history_selected = 0;
        self.history_list_state.select(Some(0));
    }

    pub fn history_last(&mut self) {
        if !self.history_events.is_empty() {
            self.history_selected = self.history_events.len() - 1;
            self.history_list_state.select(Some(self.history_selected));
        }
    }

    pub fn history_scroll_left(&mut self) {
        self.history_scroll_x = self.history_scroll_x.saturating_sub(4);
    }

    pub fn history_scroll_right(&mut self) {
        // Total row width: 16 + 40 + 17 + 14 + 20 + 24 = 131
        // Cap scroll to show at least ~45 chars (branch + id columns)
        const MAX_SCROLL: u16 = 87;
        self.history_scroll_x = (self.history_scroll_x.saturating_add(4)).min(MAX_SCROLL);
    }

    pub fn get_task_title(&self, id: &str) -> Option<&str> {
        self.all_tasks.get(id).map(|t| t.title.as_str())
    }

    pub fn toggle_history_detail(&mut self) {
        self.history_show_detail = !self.history_show_detail;
        self.history_detail_scroll = 0;
    }

    pub fn close_history_detail(&mut self) {
        if self.history_show_detail {
            self.history_show_detail = false;
        }
    }

    pub fn history_detail_scroll_down(&mut self) {
        self.history_detail_scroll = self.history_detail_scroll.saturating_add(1);
    }

    pub fn history_detail_scroll_up(&mut self) {
        self.history_detail_scroll = self.history_detail_scroll.saturating_sub(1);
    }

    pub fn selected_history_event(&self) -> Option<&Event> {
        self.history_events.get(self.history_selected)
    }

    pub fn get_task(&self, id: &str) -> Option<&Task> {
        self.all_tasks.get(id)
    }
}
