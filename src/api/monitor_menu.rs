//! The monitor's own proposal on its drop's menu (§FS-021-monitors.16): the
//! `workflow` or `action` a monitor names heads the menu of its open drop,
//! ahead of what ephor recognized, the project's offers and the person's
//! actions, so it is the proposal (§FS-019-waiting.7.1). An unknown matter,
//! and a drop whose monitor names nothing, keep the ordinary menu
//! (§FS-021-monitors.16.2). Nothing here dispatches (§FS-021-monitors.16.3).

use crate::feed::config::{ActionConfig, WorkflowAsk};
use crate::feed::model::Item;
use crate::monitors::model::Proposal;

/// The id of the row that says a monitor's `action` was passed over.
pub const PASSED_OVER: &str = "monitor-proposal";

/// Puts the monitor's proposal at the head of `menu`, before the menu is
/// gated: the entry its `action` names, or the one laying its `workflow`
/// down, made where the menu holds none. An `action` naming no entry that
/// hands work over is passed over, and a withheld row at the head says why
/// (§FS-021-monitors.16.1).
pub fn lead(menu: &mut Vec<ActionConfig>, item: &Item) {
    let Some(stated) = crate::monitors::matters::stated(&item.source, &item.raw) else {
        return;
    };
    let Some(proposal) = stated.proposes.filter(|_| !stated.terminal) else {
        return;
    };
    let hands = |entry: &ActionConfig| entry.agent.is_some() || entry.workflow.is_some();
    let at = match &proposal {
        Proposal::Workflow(name) => menu.iter().position(|entry| {
            entry.agent.is_none()
                && entry
                    .workflow
                    .as_ref()
                    .is_some_and(|workflow| &workflow.name == name)
        }),
        Proposal::Action(id) => menu
            .iter()
            .position(|entry| &entry.id == id && hands(entry)),
    };
    let head = match (at, proposal) {
        (Some(at), _) => menu.remove(at),
        (None, Proposal::Workflow(name)) => {
            menu.retain(|entry| entry.id != name);
            workflow(name)
        }
        (None, Proposal::Action(id)) => passed_over(&id),
    };
    menu.insert(0, head);
}

/// The entry laying the monitor's workflow down about its matter.
fn workflow(name: String) -> ActionConfig {
    ActionConfig {
        id: name.clone(),
        icon: crate::work::workflow::WORKFLOW_ICON.to_string(),
        description: format!("{name} (the monitor's workflow)"),
        workflow: Some(WorkflowAsk {
            name,
            ..WorkflowAsk::default()
        }),
        ..ActionConfig::default()
    }
}

/// Why the monitor's `action` is not the proposal, where the menu shows it.
pub fn passed_over(id: &str) -> ActionConfig {
    ActionConfig {
        id: PASSED_OVER.to_string(),
        icon: "·".to_string(),
        description: format!("the monitor's action `{id}`"),
        withheld: Some(format!(
            "the monitor proposes `{id}`, and no entry by that name hands work over"
        )),
        ..ActionConfig::default()
    }
}

#[cfg(test)]
#[path = "monitor_menu_tests.rs"]
mod tests;
