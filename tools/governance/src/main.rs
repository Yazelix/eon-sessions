use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const CONTRACT_STATUSES: &[&str] = &["Planned", "Partially proved", "Proved", "Retired"];
const DECISION_STATUSES: &[&str] = &["Selected", "Planned", "Candidate", "Deferred"];
const PORTABILITY: &[&str] = &["neutral", "isolated platform dependency", "macos blocker"];

#[derive(Debug, Deserialize)]
struct Comment {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct Issue {
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    issue_type: String,
    #[serde(default)]
    labels: Vec<String>,
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
    #[serde(default)]
    comments: Vec<Comment>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CheckResult {
    errors: Vec<String>,
    protocol_exceptions: Vec<String>,
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
    let mut seen = BTreeSet::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Issue>(line) {
            Ok(issue) => {
                if !seen.insert(issue.id.clone()) {
                    errors.push(format!(
                        ".beads/issues.jsonl: duplicate Bead ID {}",
                        issue.id
                    ));
                }
                issues.push(issue);
            }
            Err(error) => errors.push(format!(
                ".beads/issues.jsonl:{}: invalid issue JSON: {error}",
                number + 1
            )),
        }
    }
    issues
}

fn issue_text(issue: &Issue) -> String {
    let mut text = [
        issue.description.as_str(),
        issue.design.as_str(),
        issue.acceptance_criteria.as_str(),
        issue.notes.as_str(),
        issue.close_reason.as_str(),
    ]
    .join("\n");
    for comment in &issue.comments {
        text.push('\n');
        text.push_str(&comment.text);
    }
    text
}

fn is_implementation(issue: &Issue) -> bool {
    matches!(
        issue.issue_type.as_str(),
        "bug" | "chore" | "feature" | "task"
    ) && !issue.labels.iter().any(|label| label == "spike")
}

fn contains_non_product(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    [
        "changes no product contract",
        "changed no product contract",
        "changes no product-contract",
        "changed no product-contract",
        "no orb-c product contract",
        "no orb-c product-contract",
        "tooling only",
        "non-product",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn contains_crate_marker(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    if text.contains("docs/crates.md")
        || ((text.contains("cargo.toml")
            || text.contains("cargo.lock")
            || text.contains("cargo files"))
            && text.contains("unchanged"))
    {
        return true;
    }
    text.match_indices("no").any(|(position, _)| {
        let before = position
            .checked_sub(1)
            .and_then(|index| text.as_bytes().get(index))
            .is_some_and(u8::is_ascii_alphanumeric);
        if before {
            return false;
        }
        let suffix = text[position..].chars().take(100).collect::<String>();
        suffix.contains("crate")
            || suffix.contains("dependency")
            || suffix.contains("manifest change")
    })
}

fn contains_portability(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    PORTABILITY.iter().any(|marker| text.contains(marker))
}

fn check_issues(
    issues: &[Issue],
    known_contracts: &BTreeSet<String>,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut exceptions = BTreeSet::new();
    for issue in issues {
        let text = issue_text(issue);
        let references = contract_references(&text);
        for unknown in references.difference(known_contracts) {
            errors.push(format!(
                "{}: unknown contract reference {unknown}",
                issue.id
            ));
        }
        let comments = issue
            .comments
            .iter()
            .map(|comment| comment.text.as_str())
            .collect::<Vec<_>>();
        if comments
            .iter()
            .any(|comment| comment.starts_with("Protocol exception"))
        {
            exceptions.insert(issue.id.clone());
        }
        if !is_implementation(issue) {
            continue;
        }
        let non_product = contains_non_product(&text);
        if references.is_empty() && !non_product {
            errors.push(format!(
                "{}: implementation Bead lacks contract routing",
                issue.id
            ));
        }
        if issue.status != "closed" {
            continue;
        }

        let baseline = comments
            .iter()
            .rposition(|comment| comment.starts_with("Execution baseline"));
        if baseline.is_none() {
            errors.push(format!(
                "{}: closed implementation Bead is missing an Execution baseline",
                issue.id
            ));
        }
        let reference = baseline.and_then(|baseline| {
            comments
                .iter()
                .enumerate()
                .skip(baseline + 1)
                .rfind(|(_, comment)| comment.starts_with("Reference gate"))
                .map(|(index, _)| index)
        });
        if reference.is_none() {
            errors.push(format!(
                "{}: closed implementation Bead is missing a post-baseline Reference gate",
                issue.id
            ));
        }
        let closure = reference
            .map(|reference| {
                comments
                    .iter()
                    .skip(reference + 1)
                    .filter(|comment| {
                        comment.starts_with("Closure evidence")
                            || comment.starts_with("Closure measurement")
                    })
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if closure.is_empty() {
            errors.push(format!(
                "{}: closed implementation Bead is missing current Closure evidence",
                issue.id
            ));
        }
        if !closure.contains("docs/CONTRACTS.md") && !contains_non_product(&closure) {
            errors.push(format!(
                "{}: closure is missing a contract-update marker",
                issue.id
            ));
        }
        if !contains_crate_marker(&closure) {
            errors.push(format!(
                "{}: closure is missing a crate-decision marker",
                issue.id
            ));
        }
        if !non_product && !contains_portability(&closure) {
            errors.push(format!(
                "{}: closure is missing a portability disposition",
                issue.id
            ));
        }
    }
    exceptions.into_iter().collect()
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

fn check_repository(root: &Path) -> CheckResult {
    let mut errors = Vec::new();
    let contracts = read_text(root, "docs/CONTRACTS.md", &mut errors);
    let known_contracts = check_contracts(&contracts, &mut errors);
    let beads = read_text(root, ".beads/issues.jsonl", &mut errors);
    let issues = load_issues(&beads, &mut errors);
    let protocol_exceptions = check_issues(&issues, &known_contracts, &mut errors);
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
    CheckResult {
        errors,
        protocol_exceptions,
    }
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
    let result = check_repository(&root);
    for bead_id in result.protocol_exceptions {
        println!("notice: {bead_id} has a Protocol exception record");
    }
    if result.errors.is_empty() {
        println!("governance: ok");
        ExitCode::SUCCESS
    } else {
        for error in result.errors {
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
                "labels": ["runtime"],
                "description": "Preserve ORB-C1.",
                "notes": "Historical notes must not satisfy current closure.",
                "close_reason": "Completed.",
                "comments": [
                    {"text": "Execution baseline: preserve ORB-C1."},
                    {"text": "Reference gate: exact evidence inspected."},
                    {"text": "Protocol exception: visible historical record."},
                    {"text": "Closure evidence: docs/CONTRACTS.md and docs/CRATES.md updated. Portability disposition: neutral. Cargo.toml and Cargo.lock are unchanged."}
                ]
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
            check_repository(&self.root).errors
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn valid_repository_keeps_protocol_exceptions_visible() {
        let repository = TestRepository::new();

        assert_eq!(
            check_repository(&repository.root),
            CheckResult {
                errors: Vec::new(),
                protocol_exceptions: vec!["orb-proof".into()],
            }
        );
    }

    #[test]
    fn contract_failures_are_reported_together() {
        let mut repository = TestRepository::new();
        repository.write(
            "docs/CONTRACTS.md",
            &format!(
                "# Contracts\n\n## Current contracts\n\n\
                 | ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |\n\
                 | --- | --- | --- | --- | --- | --- | --- |\n\
                 | `ORB-C1` | One | Orbit | Proved | `{COMMIT}` | canonical test | None |\n\
                 | `ORB-C1` | Duplicate | Orbit | Done | `{COMMIT}` | canonical test | None |\n\
                 | `ORB-X` | Malformed | Orbit | Planned | — | plan | Open |\n\
                 | `ORB-C2` | Bad proof | Orbit | Proved | `edge` | — | None |\n"
            ),
        );
        repository.issue["description"] = json!("Preserve ORB-C1 and unknown ORB-C9.");
        repository.write_issue();

        let errors = repository.errors().join("\n");

        assert!(errors.contains("duplicate contract ID ORB-C1"));
        assert!(errors.contains("malformed contract ID `ORB-X`"));
        assert!(errors.contains("invalid contract status Done"));
        assert!(errors.contains("ORB-C2 requires a full 40-character proof commit"));
        assert!(errors.contains("ORB-C2 requires a named check or evidence"));
        assert!(errors.contains("unknown contract reference ORB-C9"));
    }

    #[test]
    fn stale_proof_and_protocol_exceptions_do_not_satisfy_current_gates() {
        let mut repository = TestRepository::new();
        repository.issue["comments"]
            .as_array_mut()
            .expect("comments array")
            .push(json!({"text": "Execution baseline: restarted work."}));
        repository.write_issue();

        let errors = repository.errors().join("\n");

        assert!(errors.contains("missing a post-baseline Reference gate"));
        assert!(errors.contains("missing current Closure evidence"));
        assert!(errors.contains("missing a contract-update marker"));
        assert!(errors.contains("missing a crate-decision marker"));
        assert!(errors.contains("missing a portability disposition"));
        assert_eq!(
            check_repository(&repository.root).protocol_exceptions,
            ["orb-proof"]
        );
    }

    #[test]
    fn selected_crate_decisions_require_complete_evidence() {
        let repository = TestRepository::new();
        repository.write(
            "docs/CRATES.md",
            "# Crates\n\n## Current decisions\n\n\
             | Boundary | Selected shape | Status | Credible alternatives | Why | Evidence |\n\
             | --- | --- | --- | --- | --- | --- |\n\
             | Runtime | Owned loop | Selected | — | Small | future work |\n\
             | Renderer | Maybe | Chosen | owned code | Deferred | orb-proof |\n",
        );

        let errors = repository.errors().join("\n");

        assert!(errors.contains("selected Runtime decision lacks an exact version or commit"));
        assert!(errors.contains("selected Runtime decision lacks credible alternatives"));
        assert!(errors.contains("selected Runtime decision lacks an existing evidence Bead"));
        assert!(errors.contains("Renderer has invalid decision status Chosen"));
    }
}
