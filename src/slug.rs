//! An id as a name a branch and a path will both take (§FS-005-dispatch.2).
//!
//! A matter's id is `<source>:<plan>.<task>` or `<source>:<owner>/<repo>#<n>`,
//! and git takes neither: a `:` is forbidden in a ref and a `#` is a comment
//! everywhere else. So a name is made from it — the readable half for whoever
//! reads the branch, and a digest of the whole id so that two ids reading down
//! to one slug stay two names.
//!
//! Reducing text to a name is pure text work with no store behind it, which is
//! why it sits in core beside [`crate::ticket_ids`] rather than in whatever
//! reads the files it names (§AR-001-layers.1, §AR-001-layers.3).

/// The readable half of a name: `text` lowercased over its ASCII
/// alphanumerics, with every other run of characters collapsed to a single `-`
/// and trimmed at both ends (§FS-005-dispatch.2). Empty where `text` holds no
/// alphanumeric at all — a caller that needs a name says what to write there.
pub fn readable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// `text` as eight hex digits (FNV-1a, 32 bits).
///
/// Written out rather than taken from the standard library's hasher, whose
/// output is explicitly not stable between releases: these digits go into an
/// id in a file on disk and into a branch a second run has to resolve to
/// again, and a digest that changed when the compiler did would make every
/// later run miss what the earlier one wrote (§FS-005-dispatch.2).
pub fn fingerprint(text: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// What `{id_slug}` renders for a matter whose id is `id`: the [`readable`]
/// half and a [`fingerprint`] of the **whole** id, always both
/// (§FS-005-dispatch.2).
///
/// The digest is unconditional rather than a tiebreaker, because rendering is
/// the resolution and nothing is written down (§FS-005-dispatch.25): there is
/// nowhere to ask whether one was needed, so a digest that appeared only on a
/// collision would mean a second matter arriving later silently changed what
/// the first already resolves to.
///
/// An id of punctuation alone leaves no readable half, and renders
/// `item-<digest>` rather than a name beginning with `-`. A leading digit is
/// kept: git and a filesystem both take one, and holding this to the runtime's
/// file-stem grammar is what would make it the matter's plan file stem, which
/// it deliberately is not.
pub fn id_slug(id: &str) -> String {
    let stem = readable(id);
    let stem = match stem.is_empty() {
        true => "item".to_string(),
        false => stem,
    };
    format!("{stem}-{}", fingerprint(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The six values §FS-005-dispatch.2 pins as literals. The spec is what
    /// they have to agree with, not this implementation: a branch minted from
    /// one is resolved to again by every later run, so a moved digit would
    /// resolve the same matter to a second workspace.
    #[test]
    fn the_name_an_id_renders_is_the_one_the_ground_pins() {
        for (id, name) in [
            ("rhei:window.1", "rhei-window-1-d8a9c768"),
            ("rhei:window.2", "rhei-window-2-dba9cc21"),
            ("rhei:window.retry-1", "rhei-window-retry-1-17bbeb3b"),
            ("rhei:window-retry.1", "rhei-window-retry-1-5ff4987f"),
            (
                "acmeforge:acme/widget#95",
                "acmeforge-acme-widget-95-4ef7cc9e",
            ),
            (
                "github-issues:agent-grounds/ephor#120",
                "github-issues-agent-grounds-ephor-120-bac79ee0",
            ),
        ] {
            assert_eq!(id_slug(id), name, "{id}");
        }
    }

    /// The third and fourth of those read down to one slug and stay two
    /// names, which is the whole reason the digest is appended every time
    /// (§FS-005-dispatch.2).
    #[test]
    fn two_ids_that_read_down_to_one_slug_are_two_names() {
        assert_eq!(
            readable("rhei:window.retry-1"),
            readable("rhei:window-retry.1")
        );
        assert_ne!(
            id_slug("rhei:window.retry-1"),
            id_slug("rhei:window-retry.1")
        );
        // And every rendering is a name git will take, whatever the id held.
        for id in [
            "rhei:window.1",
            "acmeforge:acme/widget#95",
            ":::",
            "  spaced  out  ",
            "HEAD",
        ] {
            let name = id_slug(id);
            assert!(
                name.chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
                    && !name.starts_with('-'),
                "{id} rendered {name}"
            );
            assert_eq!(crate::branches::why_git_refuses(&name), None, "{id}");
        }
    }

    /// An id with no readable half at all still renders a name, and one
    /// beginning with a digit keeps it — which is where this and the matter's
    /// plan file stem deliberately part company (§FS-005-dispatch.2).
    #[test]
    fn an_id_with_nothing_readable_is_still_a_name_and_a_digit_stays() {
        assert_eq!(id_slug(":::"), "item-20bed5dd");
        assert_eq!(id_slug(""), "item-811c9dc5");
        // `plan_id` would write `item-42` here, because the runtime's
        // file-stem grammar refuses a stem beginning with a digit. Neither git
        // nor a filesystem does, so this field keeps it.
        assert_eq!(id_slug("42"), "42-87e38583");
        assert_eq!(readable("///"), "");
    }
}
