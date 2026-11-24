//! Local gate decisions and review-file readings (§FS-005-dispatch.9.1), and
//! a prepared gate's answer (§FS-005-dispatch.9.3).

use std::process::ExitCode;

use crate::api::Session;
use crate::cli::{WorkGateAnswerArgs, WorkReviewArgs, WorkTransitionArgs};
use crate::error::{registry_error, EphorError, Result};
use crate::feed::config::StatusConfig;
use crate::work::runtime::human_gate;

pub(super) fn review(config: &StatusConfig, args: &WorkReviewArgs) -> Result<ExitCode> {
    let session = Session::open(config)?;
    let item = session
        .item(&args.item)
        .ok_or_else(|| registry_error(format!("'{}' is not in any cached feed", args.item)))?;
    let review = session.human_review(&item)?;
    if let Some(number) = args.file {
        let file = number
            .checked_sub(1)
            .and_then(|index| review.files.get(index))
            .ok_or_else(|| EphorError::Command(format!("No review file numbered {number}")))?;
        let text = human_gate::read_file(file)?;
        if args.json {
            println!(
                "{}",
                serde_json::json!({"name": file.name, "path": file.path, "text": text})
            );
        } else {
            print!("{text}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&review)
                .map_err(|error| EphorError::Command(error.to_string()))?
        );
        return Ok(ExitCode::SUCCESS);
    }
    // With no live gate the sentence stays, and any finished blocked files
    // are listed under it (§FS-005-dispatch.9.2).
    if review.gates.is_empty() {
        println!("{}", human_gate::NO_GATE);
    }
    for gate in &review.gates {
        println!("{} [{}] {}", gate.ticket, gate.state, gate.title);
        // The brief in one line, where the runtime prepared one; its
        // questions are read with `--json` and answered with `work gate
        // answer` (§FS-005-dispatch.9.3).
        if let Some(brief) = &gate.brief {
            println!(
                "  brief: {} · {} question(s){} · answer with `ephor work gate answer`",
                brief.title,
                brief.questions.len(),
                match brief.asks_its_own() {
                    true => ", required ones of its own",
                    false => "",
                }
            );
        } else if let Some(unbriefed) = &gate.unbriefed {
            println!("  no brief: {unbriefed}");
        }
        for destination in &gate.destinations {
            println!(
                "  → {}{}{}",
                destination.state,
                destination
                    .description
                    .as_ref()
                    .map(|description| format!(" — {description}"))
                    .unwrap_or_default(),
                destination
                    .condition
                    .as_ref()
                    .map(|condition| format!(" (runtime checks: {condition})"))
                    .unwrap_or_default()
            );
        }
    }
    for (index, file) in review.files.iter().enumerate() {
        println!(
            "  file {}: {} — {}{}",
            index + 1,
            file.name,
            file.path.display(),
            file.problem
                .as_ref()
                .map(|problem| format!(" ({problem})"))
                .unwrap_or_default()
        );
    }
    Ok(ExitCode::SUCCESS)
}

pub(super) fn transition(config: &StatusConfig, args: &WorkTransitionArgs) -> Result<ExitCode> {
    let outcome = (|| {
        let session = Session::open(config)?;
        let item = session
            .item(&args.item)
            .ok_or_else(|| registry_error(format!("'{}' is not in any cached feed", args.item)))?;
        session.transition_work(&item, &args.ticket, &args.from, &args.to, &args.result)
    })();
    if args.json {
        let mut report = serde_json::json!({
            "item": args.item, "ticket": args.ticket, "from": args.from, "to": args.to,
            "transitioned": outcome.is_ok(),
        });
        if let Err(error) = &outcome {
            report["says"] = error.to_string().into();
        }
        println!("{report}");
    } else {
        match &outcome {
            Ok(()) => println!("{}: {} → {}", args.ticket, args.from, args.to),
            Err(error) => eprintln!("Refused: {error}"),
        }
    }
    Ok(if outcome.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// One answer to a prepared gate: the document read from a file or standard
/// input and sent to the runtime unchanged, its verdict printed as it came
/// back (§FS-005-dispatch.9.3). A refusal exits non-zero, the verdict still on
/// standard output under `--json`.
pub(super) fn answer(config: &StatusConfig, args: &WorkGateAnswerArgs) -> Result<ExitCode> {
    let outcome = (|| {
        let document = answer_document(&args.answers)?;
        let session = Session::open(config)?;
        let item = session
            .item(&args.item)
            .ok_or_else(|| registry_error(format!("'{}' is not in any cached feed", args.item)))?;
        session.answer_gate(&item, &args.ticket, &args.from, &document, args.dry_run)
    })();
    let mut report = serde_json::json!({
        "item": args.item, "ticket": args.ticket, "from": args.from, "dry_run": args.dry_run,
    });
    let ok = match &outcome {
        Ok(verdict) => {
            report["dry_run"] = (verdict.dry_run || args.dry_run).into();
            report["ok"] = verdict.ok.into();
            report["to"] = serde_json::json!(verdict.to);
            report["route"] = verdict.route.clone().unwrap_or_default();
            report["answer_path"] = serde_json::json!(verdict.answer_path);
            report["result"] = serde_json::json!(verdict.result);
            report["errors"] = serde_json::json!(verdict.errors);
            if let Some(says) = verdict.says() {
                report["says"] = says.into();
            }
            verdict.ok
        }
        Err(error) => {
            report["ok"] = false.into();
            report["errors"] = serde_json::json!([]);
            report["says"] = error.to_string().into();
            false
        }
    };
    if args.json {
        println!("{report}");
    } else {
        match &outcome {
            Ok(verdict) if verdict.ok => {
                let to = verdict.to.as_deref().unwrap_or("unknown");
                match args.dry_run {
                    true => println!(
                        "{}: {} → {to} (dry run; nothing moved)",
                        args.ticket, args.from
                    ),
                    false => println!("{}: {} → {to}", args.ticket, args.from),
                }
                if let Some(path) = &verdict.answer_path {
                    println!("  answer recorded at {path}");
                }
            }
            Ok(verdict) => {
                eprintln!("Refused: {}", verdict.says().unwrap_or_default());
            }
            Err(error) => eprintln!("Refused: {error}"),
        }
    }
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// The answer document, from a file or `-` for standard input. Read as JSON
/// so it can be handed on whole; its shape is the runtime's to judge.
fn answer_document(source: &str) -> Result<serde_json::Value> {
    let text = if source == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
            .map_err(|error| EphorError::Command(format!("cannot read the answers: {error}")))?;
        text
    } else {
        std::fs::read_to_string(source)
            .map_err(|error| EphorError::Command(format!("cannot read {source}: {error}")))?
    };
    serde_json::from_str(&text)
        .map_err(|error| EphorError::Command(format!("the answers are not JSON: {error}")))
}
