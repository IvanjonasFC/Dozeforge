//! MacroDroid template exporter.

use serde::{Deserialize, Serialize};

use crate::optimizer::OptimizationAction;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroDroidMacro {
    pub name: String,
    pub triggers: Vec<String>,
    pub actions: Vec<MacroDroidAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroDroidAction {
    RunShellScript { script: String, use_root: bool },
    DisplayToast { message: String },
}

pub struct MacroDroidExport;

impl MacroDroidExport {
    pub fn render(actions: &[OptimizationAction]) -> Result<String, serde_json::Error> {
        let mut script = String::new();
        for action in actions {
            script.push_str(&action.to_shell());
            script.push('\n');
        }
        let macro_def = MacroDroidMacro {
            name: "DozeForge: power profile".to_string(),
            triggers: vec!["device_boot".to_string()],
            actions: vec![
                MacroDroidAction::RunShellScript { script, use_root: false },
                MacroDroidAction::DisplayToast { message: "DozeForge profile re-applied".to_string() },
            ],
        };
        serde_json::to_string_pretty(&macro_def)
    }
}
