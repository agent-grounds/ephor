//! An isolated process environment for standing tests (§AR-009-surfaces.2).
//! Reuse the integration harness's canonical temp paths and registry helpers.

use super::common;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const PROJECT: &str = "demo";

pub struct World(tempfile::TempDir);

impl World {
    pub fn new() -> Self {
        let world = Self(common::tempdir());
        fs::create_dir_all(world.forest()).unwrap();
        fs::create_dir_all(world.path().join("fakebin")).unwrap();
        world
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }
    pub fn forest(&self) -> PathBuf {
        self.path().join(PROJECT)
    }

    pub fn register(&self, row: Value) {
        let template = common::write_template(self.path());
        let mut project = json!({"id": PROJECT, "type": "monorepo",
            "display_name": "Demo", "root": self.forest(), "main_branch": "main"});
        project
            .as_object_mut()
            .unwrap()
            .extend(row.as_object().unwrap().clone());
        common::write_registry(
            &self.path().join("workspaces.json"),
            &json!({
                "project_types": common::base_project_types(&template),
                "hook_sets": [], "projects": [project]
            }),
        );
    }

    pub fn configure(&self, config: Value) {
        fs::write(self.path().join("status.json"), config.to_string()).unwrap();
    }

    pub fn ephor_raw(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("ephor"));
        command
            .current_dir(self.path())
            .env("HOME", self.path())
            .env("XDG_STATE_HOME", self.path().join("state"))
            .env("EPHOR_REGISTRY", self.path().join("workspaces.json"))
            .env("EPHOR_STATUS_CONFIG", self.path().join("status.json"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.path().join("fakebin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            );
        command
    }

    pub fn ephor(&self) -> assert_cmd::Command {
        assert_cmd::Command::from_std(self.ephor_raw())
    }

    pub fn stub(&self, name: &str, body: &str) {
        common::make_executable(&self.path().join("fakebin").join(name), body);
    }

    pub fn matters(&self) -> Vec<Value> {
        let feed: Value = serde_json::from_str(
            &fs::read_to_string(self.path().join("state/ephor/feed/demo.json")).unwrap(),
        )
        .unwrap();
        feed["providers"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|slot| slot["matters"].as_array().unwrap().clone())
            .collect()
    }
}
