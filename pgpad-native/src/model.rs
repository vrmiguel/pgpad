#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Connections,
    Items,
    Scripts,
    History,
}

impl SidebarTab {
    pub const ALL: [Self; 4] = [Self::Connections, Self::Items, Self::Scripts, Self::History];

    pub fn label(self) -> &'static str {
        match self {
            Self::Connections => "Connections",
            Self::Items => "Items",
            Self::Scripts => "Scripts",
            Self::History => "History",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Postgres,
    Sqlite,
}

/// Dummy for now while the POC isn't wired up to pgpad-core
#[derive(Debug, Clone)]
pub struct DummyConnection {
    pub name: &'static str,
    pub detail: &'static str,
    pub kind: ConnectionKind,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub struct ScriptTab {
    pub title: String,
    pub dirty: bool,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ResultTab {
    pub title: &'static str,
}

#[derive(Debug)]
pub struct AppModel {
    pub sidebar_collapsed: bool,
    pub active_sidebar_tab: SidebarTab,
    pub selected_connection: usize,
    pub active_script: usize,
    pub active_result: usize,
    pub selected_result_row: Option<usize>,
    pub connections: Vec<DummyConnection>,
    pub scripts: Vec<ScriptTab>,
    pub results: Vec<ResultTab>,
}

impl Default for AppModel {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            active_sidebar_tab: SidebarTab::Connections,
            selected_connection: 5,
            active_script: 4,
            active_result: 0,
            selected_result_row: None,
            connections: vec![
                DummyConnection {
                    name: "Local",
                    detail: "localhost:5432",
                    kind: ConnectionKind::Postgres,
                    connected: true,
                },
                DummyConnection {
                    name: "Chinook",
                    detail: "Chinook_Sqlite.sqlite",
                    kind: ConnectionKind::Sqlite,
                    connected: false,
                },
            ],
            scripts: vec![
                ScriptTab {
                    title: "RestoreTemboGH".to_owned(),
                    dirty: false,
                    text: "select 5".to_owned(),
                },
                ScriptTab {
                    title: "StreamEvent".to_owned(),
                    dirty: true,
                    text: "select 4".to_owned(),
                },
                ScriptTab {
                    title: "Call".to_owned(),
                    dirty: true,
                    text: "select 3".to_owned(),
                },
                ScriptTab {
                    title: "Tessl-billing-ch...".to_owned(),
                    dirty: true,
                    text: "select 2".to_owned(),
                },
                ScriptTab {
                    title: "Untitled Script 36".to_owned(),
                    dirty: false,
                    text: "select 1".to_owned(),
                },
            ],
            results: vec![ResultTab {
                title: "select distinct provider fr...",
            }],
        }
    }
}

impl AppModel {
    pub fn selected_connection(&self) -> Option<&DummyConnection> {
        self.connections.get(self.selected_connection)
    }

    pub fn active_script_mut(&mut self) -> Option<&mut ScriptTab> {
        self.scripts.get_mut(self.active_script)
    }

    pub fn select_script(&mut self, idx: usize) {
        if idx < self.scripts.len() {
            self.active_script = idx;
        }
    }

    pub fn add_script(&mut self) {
        let next = self.scripts.len() + 36;
        self.scripts.push(ScriptTab {
            title: format!("Untitled Script {next}"),
            dirty: false,
            text: "-- New scratch query\nselect now();".to_owned(),
        });
        self.active_script = self.scripts.len() - 1;
    }

    pub fn close_script(&mut self, idx: usize) {
        if self.scripts.len() <= 1 || idx >= self.scripts.len() {
            return;
        }

        self.scripts.remove(idx);
        self.active_script = self.active_script.min(self.scripts.len() - 1);
    }
}

pub fn result_columns() -> [&'static str; 5] {
    [
        "provider",
        "requests",
        "avg_latency_ms",
        "last_seen",
        "status",
    ]
}

pub fn result_rows() -> Vec<[String; 5]> {
    vec![
        [
            "anthropic".to_owned(),
            "1,482".to_owned(),
            "842".to_owned(),
            "2026-05-26 11:31:44".to_owned(),
            "healthy".to_owned(),
        ],
        [
            "openai_responses".to_owned(),
            "6,021".to_owned(),
            "615".to_owned(),
            "2026-05-26 11:34:02".to_owned(),
            "healthy".to_owned(),
        ],
        [
            "tembo".to_owned(),
            "293".to_owned(),
            "1,204".to_owned(),
            "2026-05-26 10:59:18".to_owned(),
            "watch".to_owned(),
        ],
        [
            "local_sqlite".to_owned(),
            "74".to_owned(),
            "12".to_owned(),
            "2026-05-25 21:13:09".to_owned(),
            "idle".to_owned(),
        ],
    ]
}
