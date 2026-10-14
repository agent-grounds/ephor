//! Local gate decisions and review-file readings (§FS-005-dispatch.9.1).

use std::process::ExitCode;

use crate::api::Session;
use crate::cli::{WorkReviewArgs, WorkTransitionArgs};
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
    if review.gates.is_empty() {
        println!("No work is waiting at a human gate for this matter.");
    }
    for gate in &review.gates {
        println!("{} [{}] {}", gate.ticket, gate.state, gate.title);
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
