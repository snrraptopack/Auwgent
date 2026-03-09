pub(crate) fn find_closest<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
    if candidates.is_empty() {
        return None;
    }

    let mut best_match = None;
    let mut min_dist = usize::MAX;

    for &candidate in candidates {
        let dist = levenshtein(target, candidate);
        let threshold = (target.len() / 3).max(1) + 1;
        if dist < min_dist && dist <= threshold {
            min_dist = dist;
            best_match = Some(candidate);
        }
    }

    best_match
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let len_a = a.len();
    let len_b = b.len();

    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }

    let mut row: Vec<usize> = (0..=len_b).collect();

    for (i, &char_a) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;

        for (j, &char_b) in b.iter().enumerate() {
            let old_val = row[j + 1];
            let cost = if char_a == char_b { 0 } else { 1 };

            row[j + 1] = std::cmp::min(
                std::cmp::min(
                    row[j] + 1,
                    row[j + 1] + 1,
                ),
                prev + cost,
            );

            prev = old_val;
        }
    }

    row[len_b]
}

pub(crate) fn extract_template_condition_refs(condition: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let operators = ["==", "!=", ">=", "<=", ">", "<"];

    let mut matched = false;
    for operator in operators {
        if let Some((left, right)) = condition.split_once(operator) {
            matched = true;
            collect_template_ref(left, &mut refs);
            collect_template_ref(right, &mut refs);
            break;
        }
    }

    if !matched {
        collect_template_ref(condition, &mut refs);
    }

    refs
}

fn collect_template_ref(token: &str, refs: &mut Vec<String>) {
    let trimmed = token
        .trim()
        .trim_matches(|c: char| matches!(c, '(' | ')' | '{' | '}' | '[' | ']'));

    if trimmed.is_empty()
        || trimmed == "true"
        || trimmed == "false"
        || trimmed.parse::<f64>().is_ok()
        || ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return;
    }

    let root = trimmed.split('.').next().unwrap_or(trimmed).trim();
    if !root.is_empty() {
        refs.push(root.to_string());
    }
}