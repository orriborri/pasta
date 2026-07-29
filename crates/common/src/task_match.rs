//! Task-rule matching engine.
//!
//! Evaluates the `[kb.task_rules]` config against a record's attributes to
//! decide whether pasta should auto-create a task, ask the user, or skip it.
//!
//! Match expression grammar (no parentheses; precedence NOT > AND > OR):
//! ```text
//! expr   := and ( " OR " and )*
//! and    := atom ( " AND " atom )*
//! atom   := "NOT " atom | field ":" value
//! ```
//! A `value` may contain `*` as a wildcard. Matching is case-insensitive and an
//! atom is true when the named field has any value matching the pattern.

use std::collections::HashMap;

use crate::config::{TaskRule, TaskRules};

/// What to do with a record after rule evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Create a task file automatically (untagged → Kanban "Uncategorized").
    AutoCreate,
    /// Surface in the daily note with a "create task?" checkbox.
    Ask,
    /// Drop silently.
    Skip,
    /// No rule matched.
    None,
}

/// Matchable attributes of a record: field name → one or more values.
#[derive(Debug, Default, Clone)]
pub struct MatchAttrs {
    map: HashMap<String, Vec<String>>,
}

impl MatchAttrs {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single-valued field (ignored when empty).
    #[must_use]
    pub fn with(mut self, field: &str, value: &str) -> Self {
        if !value.is_empty() {
            self.map.entry(field.to_ascii_lowercase()).or_default().push(value.to_ascii_lowercase());
        }
        self
    }

    /// Add a multi-valued field (e.g. labels/participants).
    #[must_use]
    pub fn with_many(mut self, field: &str, values: &[String]) -> Self {
        let entry = self.map.entry(field.to_ascii_lowercase()).or_default();
        for v in values {
            if !v.is_empty() {
                entry.push(v.to_ascii_lowercase());
            }
        }
        self
    }

    fn matches_atom(&self, field: &str, pattern: &str) -> bool {
        self.map.get(field).is_some_and(|values| {
            values.iter().any(|v| wildcard_match(pattern, v))
        })
    }
}

/// Classify a record's attributes against the configured rules.
///
/// Lists are evaluated in order — `auto_create`, then `ask`, then `skip` — and
/// the first matching rule wins (mirroring the documented TOML ordering).
/// Returns the chosen action and the matched rule's `signal` label, if any.
#[must_use]
pub fn classify(rules: &TaskRules, source: &str, attrs: &MatchAttrs) -> (Action, Option<String>) {
    let ordered = [
        (Action::AutoCreate, &rules.auto_create),
        (Action::Ask, &rules.ask),
        (Action::Skip, &rules.skip),
    ];
    for (action, list) in ordered {
        if let Some(rule) = first_match(list, source, attrs) {
            return (action, Some(rule.signal.clone()));
        }
    }
    (Action::None, None)
}

fn first_match<'a>(rules: &'a [TaskRule], source: &str, attrs: &MatchAttrs) -> Option<&'a TaskRule> {
    let source = source.to_ascii_lowercase();
    rules.iter().find(|r| {
        (r.source == "*" || r.source.eq_ignore_ascii_case(&source)) && eval(&r.match_expr, attrs)
    })
}

/// Evaluate a full match expression.
#[must_use]
pub fn eval(expr: &str, attrs: &MatchAttrs) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return true; // empty match = always applies to its source
    }
    expr.split(" OR ").any(|or_term| {
        or_term.split(" AND ").all(|and_term| eval_atom(and_term.trim(), attrs))
    })
}

fn eval_atom(atom: &str, attrs: &MatchAttrs) -> bool {
    if let Some(rest) = atom.strip_prefix("NOT ") {
        return !eval_atom(rest.trim(), attrs);
    }
    atom.split_once(':').is_some_and(|(field, pattern)| {
        attrs.matches_atom(&field.trim().to_ascii_lowercase(), &pattern.trim().to_ascii_lowercase())
    })
}

/// Case-insensitive glob match supporting `*` wildcards. Inputs are assumed
/// already lowercased by the caller.
fn wildcard_match(pattern: &str, value: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == value;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // Leading anchor.
            if !value[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == parts.len() - 1 {
            // Trailing anchor.
            return value[pos..].ends_with(part);
        } else if let Some(found) = value[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TaskRule, TaskRules};

    fn rule(source: &str, signal: &str, expr: &str) -> TaskRule {
        TaskRule { source: source.into(), signal: signal.into(), match_expr: expr.into() }
    }

    #[test]
    fn atom_field_value() {
        let attrs = MatchAttrs::new().with("source", "slack");
        assert!(eval("source:slack", &attrs));
        assert!(!eval("source:gmail", &attrs));
    }

    #[test]
    fn and_or_not() {
        let attrs = MatchAttrs::new().with("is", "dm").with_many("labels", &["question".into()]);
        assert!(eval("is:dm AND labels:question", &attrs));
        assert!(!eval("is:dm AND labels:budget", &attrs));
        assert!(eval("is:channel OR is:dm", &attrs));
        assert!(eval("NOT labels:bot", &attrs));
        assert!(!eval("NOT is:dm", &attrs));
    }

    #[test]
    fn wildcard_values() {
        let attrs = MatchAttrs::new().with("author", "deploy-bot");
        assert!(eval("author:*-bot", &attrs));
        assert!(!eval("author:*-human", &attrs));
        let attrs2 = MatchAttrs::new().with("author", "gitlab-bot-prod");
        assert!(eval("author:gitlab-*", &attrs2));
        assert!(eval("author:*bot*", &attrs2));
    }

    #[test]
    fn classify_first_match_wins() {
        let rules = TaskRules {
            auto_create: vec![rule("linear", "assigned", "assignee:me")],
            ask: vec![rule("slack", "mention", "is:channel")],
            skip: vec![rule("*", "bot", "author:*-bot")],
        };

        let assigned = MatchAttrs::new().with("assignee", "me");
        assert_eq!(classify(&rules, "linear", &assigned).0, Action::AutoCreate);

        let mention = MatchAttrs::new().with("is", "channel");
        assert_eq!(classify(&rules, "slack", &mention).0, Action::Ask);

        let bot = MatchAttrs::new().with("author", "deploy-bot");
        assert_eq!(classify(&rules, "gmail", &bot).0, Action::Skip);

        let nothing = MatchAttrs::new().with("source", "git");
        assert_eq!(classify(&rules, "git", &nothing).0, Action::None);
    }

    #[test]
    fn source_must_match() {
        let rules = TaskRules {
            auto_create: vec![rule("linear", "assigned", "assignee:me")],
            ask: vec![],
            skip: vec![],
        };
        let attrs = MatchAttrs::new().with("assignee", "me");
        // Same attrs but wrong source → no match.
        assert_eq!(classify(&rules, "slack", &attrs).0, Action::None);
    }
}
