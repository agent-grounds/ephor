use super::*;
use crate::feed::model::ItemKind;

fn drop(proposes: Option<Proposal>, terminal: bool) -> Item {
    let stated = crate::monitors::matters::Stated {
        terminal,
        proposes,
        ..Default::default()
    };
    Item {
        id: "monitor:widget/coverage@20261125T060000Z".into(),
        project: "widget".into(),
        source: crate::monitors::matters::SOURCE.into(),
        kind: ItemKind::Status,
        role: None,
        title: "Line coverage dropped: 79.2% (healthy ≥ 80%)".into(),
        url: None,
        state: Some("down".into()),
        needs_response: false,
        updated_at: chrono::Utc::now(),
        raw: serde_json::json!({ crate::monitors::matters::RECORD: stated }),
    }
}

fn entry(id: &str, workflow: Option<&str>) -> ActionConfig {
    ActionConfig {
        id: id.into(),
        workflow: workflow.map(|name| WorkflowAsk {
            name: name.into(),
            ..WorkflowAsk::default()
        }),
        ..ActionConfig::default()
    }
}

fn ids(menu: &[ActionConfig]) -> Vec<&str> {
    menu.iter().map(|entry| entry.id.as_str()).collect()
}

/// §FS-021-monitors.16.1: the monitor's workflow heads the menu, ahead of
/// what ephor recognized, made where no entry lays it down and moved where
/// one does.
#[test]
fn the_monitors_workflow_heads_the_menu() {
    let mut menu = vec![entry("open", None), entry("triage-it", Some("triage"))];
    let item = drop(Some(Proposal::Workflow("fix-coverage".into())), false);
    lead(&mut menu, &item);
    assert_eq!(ids(&menu), ["fix-coverage", "open", "triage-it"]);
    assert_eq!(
        menu[0].workflow.as_ref().map(|w| w.name.as_str()),
        Some("fix-coverage")
    );

    let mut menu = vec![entry("open", None), entry("cover", Some("fix-coverage"))];
    lead(&mut menu, &item);
    assert_eq!(ids(&menu), ["cover", "open"]);
}

/// §FS-021-monitors.16.1: an `action` heads the menu where it names an entry
/// that hands work over, and is passed over, saying why, where it does not.
#[test]
fn an_action_heads_the_menu_only_where_it_hands_work_over() {
    let mut menu = vec![entry("open", None), entry("release", Some("ready"))];
    lead(
        &mut menu,
        &drop(Some(Proposal::Action("release".into())), false),
    );
    assert_eq!(ids(&menu), ["release", "open"]);

    let mut menu = vec![entry("open", None)];
    lead(
        &mut menu,
        &drop(Some(Proposal::Action("open".into())), false),
    );
    assert_eq!(ids(&menu), [PASSED_OVER, "open"]);
    assert!(menu[0].withheld.as_deref().unwrap().contains("`open`"));
}

/// §FS-021-monitors.16.2: a matter whose monitor names nothing, a finished
/// one, and any other source's matter keep the ordinary menu.
#[test]
fn without_a_proposal_the_menu_is_the_ordinary_one() {
    let menu = vec![entry("open", None)];
    for item in [
        drop(None, false),
        drop(Some(Proposal::Workflow("fix".into())), true),
        Item {
            source: "custom-status".into(),
            ..drop(Some(Proposal::Workflow("fix".into())), false)
        },
    ] {
        let mut asked = menu.clone();
        lead(&mut asked, &item);
        assert_eq!(ids(&asked), ["open"]);
    }
}
