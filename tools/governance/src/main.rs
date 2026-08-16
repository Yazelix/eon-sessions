use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const CONTRACT_STATUSES: &[&str] = &["Planned", "Partially proved", "Proved", "Retired"];
const DECISION_STATUSES: &[&str] = &["Selected", "Planned", "Candidate", "Deferred"];

#[derive(Debug, Deserialize)]
struct Issue {
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    design: String,
    #[serde(default)]
    acceptance_criteria: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    close_reason: String,
}

fn read_text(root: &Path, relative: &str, errors: &mut Vec<String>) -> String {
    match fs::read_to_string(root.join(relative)) {
        Ok(text) => text,
        Err(error) => {
            errors.push(format!("{relative}: {error}"));
            String::new()
        }
    }
}

fn split_markdown_row(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    let content = line.strip_prefix('|')?.strip_suffix('|')?;
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut characters = content.chars().peekable();
    while let Some(character) = characters.next() {
        match (character, characters.peek()) {
            ('\\', Some('|')) => {
                cell.push('|');
                characters.next();
            }
            ('|', _) => {
                cells.push(cell.trim().to_owned());
                cell.clear();
            }
            _ => cell.push(character),
        }
    }
    cells.push(cell.trim().to_owned());
    Some(cells)
}

fn valid_separator(cell: &str) -> bool {
    let cell = cell.strip_prefix(':').unwrap_or(cell);
    let cell = cell.strip_suffix(':').unwrap_or(cell);
    cell.len() >= 3 && cell.bytes().all(|byte| byte == b'-')
}

fn markdown_table(
    text: &str,
    heading: &str,
    headers: &[&str],
    relative: &str,
    errors: &mut Vec<String>,
) -> Vec<Vec<String>> {
    let lines = text.lines().collect::<Vec<_>>();
    let Some(mut position) = lines.iter().position(|line| *line == heading) else {
        errors.push(format!("{relative}: missing {heading} table"));
        return Vec::new();
    };
    position += 1;
    while position < lines.len() && !lines[position].trim_start().starts_with('|') {
        position += 1;
    }
    let mut table = Vec::new();
    while position < lines.len() && lines[position].trim_start().starts_with('|') {
        if let Some(row) = split_markdown_row(lines[position]) {
            table.push(row);
        }
        position += 1;
    }
    if table.len() < 2
        || table[0]
            .iter()
            .map(String::as_str)
            .ne(headers.iter().copied())
    {
        errors.push(format!("{relative}: {heading} has unexpected columns"));
        return Vec::new();
    }
    if table[1].len() != headers.len() || !table[1].iter().all(|cell| valid_separator(cell)) {
        errors.push(format!(
            "{relative}: {heading} has an invalid separator row"
        ));
        return Vec::new();
    }
    let mut rows = Vec::new();
    for (number, row) in table.into_iter().skip(2).enumerate() {
        if row.len() == headers.len() {
            rows.push(row);
        } else {
            errors.push(format!(
                "{relative}: {heading} row {} has the wrong column count",
                number + 1
            ));
        }
    }
    rows
}

fn contract_id(cell: &str) -> Option<&str> {
    let id = cell.strip_prefix('`')?.strip_suffix('`')?;
    let number = id.strip_prefix("ORB-C")?;
    if !number.is_empty()
        && !number.starts_with('0')
        && number.bytes().all(|byte| byte.is_ascii_digit())
    {
        Some(id)
    } else {
        None
    }
}

fn lowercase_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn check_contracts(text: &str, errors: &mut Vec<String>) -> BTreeSet<String> {
    let rows = markdown_table(
        text,
        "## Current contracts",
        &[
            "ID",
            "Contract",
            "Owner",
            "Status",
            "Accepted proof revision",
            "Check or evidence",
            "Gap",
        ],
        "docs/CONTRACTS.md",
        errors,
    );
    let mut known = BTreeSet::new();
    for row in rows {
        let Some(id) = contract_id(&row[0]) else {
            errors.push(format!(
                "docs/CONTRACTS.md: malformed contract ID {}",
                row[0]
            ));
            continue;
        };
        if !known.insert(id.to_owned()) {
            errors.push(format!("docs/CONTRACTS.md: duplicate contract ID {id}"));
        }
        if !CONTRACT_STATUSES.contains(&row[3].as_str()) {
            errors.push(format!(
                "docs/CONTRACTS.md: {id} has invalid contract status {}",
                row[3]
            ));
        }
        if row[3] == "Proved" {
            let proof = row[4]
                .strip_prefix('`')
                .and_then(|value| value.strip_suffix('`'));
            if !proof.is_some_and(|value| lowercase_hex(value, 40)) {
                errors.push(format!(
                    "docs/CONTRACTS.md: Proved {id} requires a full 40-character proof commit"
                ));
            }
            if matches!(row[5].as_str(), "" | "—" | "-") {
                errors.push(format!(
                    "docs/CONTRACTS.md: Proved {id} requires a named check or evidence"
                ));
            }
        }
    }
    known
}

fn contract_references(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut references = BTreeSet::new();
    for (start, _) in text.match_indices("ORB-C") {
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        let mut end = start + "ORB-C".len();
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == start + "ORB-C".len()
            || (end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_'))
        {
            continue;
        }
        references.insert(text[start..end].to_owned());
    }
    references
}

fn load_issues(text: &str, errors: &mut Vec<String>) -> Vec<Issue> {
    let mut issues = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Issue>(line) {
            Ok(issue) => issues.push(issue),
            Err(error) => errors.push(format!(
                ".beads/issues.jsonl:{}: invalid issue JSON: {error}",
                number + 1
            )),
        }
    }
    issues
}

fn issue_text(issue: &Issue) -> String {
    [
        issue.description.as_str(),
        issue.design.as_str(),
        issue.acceptance_criteria.as_str(),
        issue.notes.as_str(),
        issue.close_reason.as_str(),
    ]
    .join("\n")
}

fn check_issue_references(
    issues: &[Issue],
    known_contracts: &BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    for issue in issues.iter().filter(|issue| issue.status != "deferred") {
        for unknown in contract_references(&issue_text(issue)).difference(known_contracts) {
            errors.push(format!(
                "{}: unknown contract reference {unknown}",
                issue.id
            ));
        }
    }
}

fn has_exact_version(text: &str) -> bool {
    text.split(|character: char| !character.is_ascii_digit() && character != '.')
        .any(|candidate| {
            let parts = candidate.split('.').collect::<Vec<_>>();
            matches!(parts.len(), 2 | 3)
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

fn has_full_commit(text: &str) -> bool {
    text.split(|character: char| !character.is_ascii_hexdigit())
        .any(|candidate| lowercase_hex(candidate, 40))
}

fn valid_bead_id(candidate: &str) -> bool {
    let Some(tail) = candidate
        .strip_prefix("orb-")
        .or_else(|| candidate.strip_prefix("ven-"))
    else {
        return false;
    };
    !tail.is_empty()
        && tail.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn bead_ids(text: &str) -> BTreeSet<String> {
    text.split(|character: char| {
        !character.is_ascii_lowercase()
            && !character.is_ascii_digit()
            && character != '-'
            && character != '.'
    })
    .filter(|candidate| valid_bead_id(candidate))
    .map(str::to_owned)
    .collect()
}

fn check_crates(text: &str, known_beads: &BTreeSet<String>, errors: &mut Vec<String>) {
    let rows = markdown_table(
        text,
        "## Current decisions",
        &[
            "Boundary",
            "Selected shape",
            "Status",
            "Credible alternatives",
            "Why",
            "Evidence",
        ],
        "docs/CRATES.md",
        errors,
    );
    for row in rows {
        let boundary = &row[0];
        let category = row[2].split_whitespace().next().unwrap_or_default();
        if !DECISION_STATUSES.contains(&category) {
            errors.push(format!(
                "docs/CRATES.md: {boundary} has invalid decision status {}",
                row[2]
            ));
            continue;
        }
        if category != "Selected" {
            continue;
        }
        if !has_exact_version(&row[1]) && !has_full_commit(&row[1]) {
            errors.push(format!(
                "docs/CRATES.md: selected {boundary} decision lacks an exact version or commit"
            ));
        }
        if matches!(row[3].as_str(), "" | "—" | "-" | "None") {
            errors.push(format!(
                "docs/CRATES.md: selected {boundary} decision lacks credible alternatives"
            ));
        }
        if bead_ids(&row[5]).is_disjoint(known_beads) {
            errors.push(format!(
                "docs/CRATES.md: selected {boundary} decision lacks an existing evidence Bead"
            ));
        }
    }
}

fn check_unknown_references(
    relative: &str,
    text: &str,
    known_contracts: &BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    for unknown in contract_references(text).difference(known_contracts) {
        errors.push(format!("{relative}: unknown contract reference {unknown}"));
    }
}

fn check_repository(root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let contracts = read_text(root, "docs/CONTRACTS.md", &mut errors);
    let known_contracts = check_contracts(&contracts, &mut errors);
    let beads = read_text(root, ".beads/issues.jsonl", &mut errors);
    let issues = load_issues(&beads, &mut errors);
    check_issue_references(&issues, &known_contracts, &mut errors);
    let crates = read_text(root, "docs/CRATES.md", &mut errors);
    check_crates(
        &crates,
        &issues.iter().map(|issue| issue.id.clone()).collect(),
        &mut errors,
    );
    check_unknown_references(
        "docs/CONTRACTS.md",
        &contracts,
        &known_contracts,
        &mut errors,
    );
    check_unknown_references("docs/CRATES.md", &crates, &known_contracts, &mut errors);
    errors
}

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let root = match (arguments.next(), arguments.next()) {
        (Some(root), None) => PathBuf::from(root),
        (None, None) => match std::env::current_dir() {
            Ok(root) => root,
            Err(error) => {
                eprintln!("error: cannot determine current directory: {error}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: orbit-governance [ROOT]");
            return ExitCode::FAILURE;
        }
    };
    let errors = check_repository(&root);
    if errors.is_empty() {
        println!("governance: ok");
        ExitCode::SUCCESS
    } else {
        for error in errors {
            eprintln!("error: {error}");
        }
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(0);

    struct TestRepository {
        root: std::path::PathBuf,
        issue: Value,
    }

    impl TestRepository {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "orbit-governance-{}-{}",
                std::process::id(),
                NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).expect("create fixture root");
            let issue = json!({
                "id": "orb-proof",
                "status": "closed",
                "issue_type": "task",
                "description": "This docs-only task preserves ORB-C1."
            });
            let repository = Self { root, issue };
            repository.write_fixture();
            repository
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        fn write_fixture(&self) {
            self.write(
                "docs/CONTRACTS.md",
                &format!(
                    "# Contracts\n\n## Current contracts\n\n\
                     | ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |\n\
                     | --- | --- | --- | --- | --- | --- | --- |\n\
                     | `ORB-C1` | Durable session | Orbit | Proved | `{COMMIT}` | canonical contract test | None |\n"
                ),
            );
            self.write(
                "docs/CRATES.md",
                "# Crates\n\n## Current decisions\n\n\
                 | Boundary | Selected shape | Status | Credible alternatives | Why | Evidence |\n\
                 | --- | --- | --- | --- | --- | --- |\n\
                 | Terminal | libghostty-vt 0.2.1 | Selected for proof | owned parser | Sole authority | orb-proof |\n\
                 | Renderer | Undecided | Planned | wgpu or owned code | Deferred | future Bead |\n",
            );
            self.write_issue();
        }

        fn write_issue(&self) {
            self.write(
                ".beads/issues.jsonl",
                &(serde_json::to_string(&self.issue).expect("serialize issue") + "\n"),
            );
        }

        fn errors(&self) -> Vec<String> {
            check_repository(&self.root)
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn valid_repository_accepts_docs_only_task_without_gate_markers() {
        let repository = TestRepository::new();
        let issue = serde_json::to_string(&repository.issue).expect("serialize issue");
        repository.write(".beads/issues.jsonl", &format!("{issue}\n{issue}\n"));

        assert!(repository.errors().is_empty());
    }

    #[test]
    fn deferred_proposals_and_archival_comments_are_not_current_contract_claims() {
        let mut repository = TestRepository::new();
        repository.issue["status"] = json!("deferred");
        repository.issue["description"] = json!("Proposed ORB-C9 requires approval.");
        repository.issue["comments"] = json!([{"text": "Historical ORB-C8 proposal."}]);
        repository.write_issue();

        assert!(repository.errors().is_empty());

        repository.issue["status"] = json!("open");
        repository.issue["description"] = json!("Preserve ORB-C1.");
        repository.write_issue();

        assert!(repository.errors().is_empty());

        repository.issue["description"] = json!("Proposed ORB-C9 requires approval.");
        repository.write_issue();

        assert!(
            repository
                .errors()
                .join("\n")
                .contains("unknown contract reference ORB-C9")
        );
    }

    #[test]
    fn invalid_metadata_failures_are_reported_together() {
        let mut repository = TestRepository::new();
        repository.write(
            "docs/CONTRACTS.md",
            &format!(
                "# Contracts\n\n## Current contracts\n\n\
                 | ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |\n\
                 | --- | --- | --- | --- | --- | --- | --- |\n\
                 | `ORB-C1` | One; see ORB-C8 | Orbit | Proved | `{COMMIT}` | canonical test | None |\n\
                 | `ORB-C1` | Duplicate | Orbit | Done | `{COMMIT}` | canonical test | None |\n\
                 | `ORB-X` | Malformed | Orbit | Planned | — | plan | Open |\n\
                 | `ORB-C2` | Bad proof | Orbit | Proved | `edge` | — | None |\n\
                 | Too | Short |\n"
            ),
        );
        repository.issue["description"] = json!("Preserve ORB-C1 and unknown ORB-C9.");
        repository.write(
            ".beads/issues.jsonl",
            &(serde_json::to_string(&repository.issue).expect("serialize issue") + "\n{\n"),
        );
        repository.write(
            "docs/CRATES.md",
            "# Crates\n\n## Current decisions\n\n\
             | Boundary | Selected shape | Status | Credible alternatives | Why | Evidence |\n\
             | --- | --- | --- | --- | --- | --- |\n\
             | Runtime | Owned loop | Selected | — | Small; preserves ORB-C7 | future work |\n\
             | Renderer | Maybe | Chosen | owned code | Deferred | orb-proof |\n\
             | Too | Short |\n",
        );

        let errors = repository.errors().join("\n");

        assert!(errors.contains("Current contracts row 5 has the wrong column count"));
        assert!(errors.contains("duplicate contract ID ORB-C1"));
        assert!(errors.contains("malformed contract ID `ORB-X`"));
        assert!(errors.contains("invalid contract status Done"));
        assert!(errors.contains("ORB-C2 requires a full 40-character proof commit"));
        assert!(errors.contains("ORB-C2 requires a named check or evidence"));
        assert!(errors.contains("unknown contract reference ORB-C9"));
        assert!(errors.contains("docs/CONTRACTS.md: unknown contract reference ORB-C8"));
        assert!(errors.contains("docs/CRATES.md: unknown contract reference ORB-C7"));
        assert!(errors.contains("invalid issue JSON"));
        assert!(errors.contains("Current decisions row 3 has the wrong column count"));
        assert!(errors.contains("selected Runtime decision lacks an exact version or commit"));
        assert!(errors.contains("selected Runtime decision lacks credible alternatives"));
        assert!(errors.contains("selected Runtime decision lacks an existing evidence Bead"));
        assert!(errors.contains("Renderer has invalid decision status Chosen"));
    }
}
