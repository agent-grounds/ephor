//! Isolated mail carrier for saved-send scenarios (§FS-005-dispatch.13).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{json, Value};

use crate::support::{read_json, shaped, write_json, World};

pub const ITEM: &str = "mail-me:k-B";
pub const H: &str = "<h@dana.example>";
pub const M: &str = "<m@dana.example>";
pub const NEW_WORDS: &str = "It is €150 now. Still interested?";

pub struct MailWorld {
    pub world: World,
}

impl MailWorld {
    pub fn new(reconciles: bool) -> Self {
        let world = World::new();
        fs::create_dir_all(world.path().join("mail")).unwrap();
        let fixture = world.path().join("reply-forge.py");
        fs::write(&fixture, include_str!("fixtures/reply-forge.py")).unwrap();
        world.stub(
            "ephor-forge-mail-me",
            &format!("#!/bin/sh\nexec python3 '{}' \"$@\"\n", fixture.display()),
        );
        world.configure(json!({
            "defaults": {"provider_timeout_seconds": 1},
            "projects": {"demo": {"providers": [{"provider": "mail-me", "user": "me"}]}}
        }));
        let this = Self { world };
        let mut capabilities = json!({"messages": true, "replies": true});
        if reconciles {
            capabilities["reply_reconciliation"] = json!(true);
        }
        this.put("capabilities.json", capabilities);
        this.put(
            "conversation.json",
            json!({
                "id": "k-B", "title": "Quote for the garden fence",
                "updated_at": "2026-10-07T09:00:00Z", "reasons": ["mentioned"],
                "threads": [{"messages": [message(H, "Can you do the fence for €100?", false)],
                    "reply": {"in_reply_to": H}}]
            }),
        );
        this.refresh();
        this
    }

    pub fn command(&self) -> std::process::Command {
        let mut command = self.world.ephor_raw();
        command.env("REPLY_WORLD", self.world.path());
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().expect("ephor runs")
    }

    pub fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "fixture command {args:?}: {}",
            says(&out)
        );
        out
    }

    pub fn refresh(&self) {
        self.ok(&["refresh", "demo"]);
    }

    pub fn reply(&self, words: Option<&str>) -> Output {
        match words {
            Some(words) => self.run(&["reply", ITEM, words]),
            None => self.run(&["reply", ITEM]),
        }
    }

    pub fn drafted(&self, words: &str) -> PathBuf {
        self.ok(&[
            "work", "dispatch", "--item", ITEM, "--recipe", "answer", "--again",
        ]);
        let path = self.requested_reply();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, words).unwrap();
        let thread = self.thread();
        assert_eq!(
            thread["draft"]["text"], words,
            "fixture draft not readable: {thread}"
        );
        path
    }

    // Use the file the brief actually asks for, as ephor.34 does, instead of
    // assuming either the old or the new path shape in the test fixture.
    pub fn requested_reply(&self) -> PathBuf {
        let ledger = read_json(&self.world.path().join("state/ephor/work.json"));
        let plan = ledger["entries"][ITEM]["plan"].as_str().unwrap();
        let text = fs::read_to_string(plan).unwrap();
        let path = ledger["entries"][ITEM]["dispatches"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["reply_path"]
            .as_str()
            .expect("request advertises {reply}");
        assert!(
            text.contains(path),
            "the brief must consume its advertised output: {text}"
        );
        PathBuf::from(path)
    }

    pub fn thread(&self) -> Value {
        shaped("thread", &self.ok(&["thread", ITEM, "--json"]))
    }

    pub fn put(&self, name: &str, value: Value) {
        write_json(&self.world.path().join("mail").join(name), &value);
    }

    pub fn get(&self, name: &str) -> Value {
        read_json(&self.world.path().join("mail").join(name))
    }

    pub fn flag(&self, name: &str) {
        fs::write(self.world.path().join("mail").join(name), "").unwrap();
    }

    pub fn incoming(&self) {
        let mut row = self.get("conversation.json");
        row["threads"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(message(M, NEW_WORDS, false));
        row["threads"][0]["reply"] = json!({"in_reply_to": M});
        row["updated_at"] = json!("2026-10-07T11:00:00Z");
        self.put("conversation.json", row);
    }

    pub fn lines(&self, name: &str) -> Vec<Value> {
        fs::read_to_string(self.world.path().join("mail").join(name))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    pub fn requests(&self) -> Vec<Value> {
        self.lines("requests.jsonl")
    }

    pub fn wires(&self) -> Vec<String> {
        fs::read_to_string(self.world.path().join("mail/requests.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    pub fn delivered(&self) -> usize {
        let path = self.world.path().join("mail/deliveries.json");
        if path.exists() {
            read_json(&path).as_array().unwrap().len()
        } else {
            0
        }
    }

    pub fn uncertain(&self, words: Option<&str>) {
        self.flag("lose-ack");
        let out = self.reply(words);
        assert!(
            !out.status.success() && says(&out).contains("timed out"),
            "fixture did not lose acknowledgement: {}",
            says(&out)
        );
        assert_eq!(
            self.requests().len(),
            1,
            "first attempt reached the carrier"
        );
        assert_eq!(
            self.delivered(),
            1,
            "carrier accepted before losing acknowledgement"
        );
    }
}

pub fn says(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

pub fn message(id: &str, text: &str, mine: bool) -> Value {
    json!({"id": id, "author": if mine {"me"} else {"dana"}, "mine": mine,
        "text": text, "when": if id == H {"2026-10-07T09:00:00Z"} else {"2026-10-07T11:00:00Z"}})
}

pub fn contains(value: &Value, needle: &Value) -> bool {
    value == needle
        || match value {
            Value::Array(values) => values.iter().any(|v| contains(v, needle)),
            Value::Object(values) => values.values().any(|v| contains(v, needle)),
            _ => false,
        }
}

pub fn tree(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(path: &Path, files: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        if !path.exists() {
            return;
        }
        if path.is_file() {
            files.insert(path.to_path_buf(), fs::read(path).unwrap());
        } else {
            for entry in fs::read_dir(path).unwrap() {
                walk(&entry.unwrap().path(), files);
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    walk(path, &mut files);
    files
}
