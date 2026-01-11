//! Fuzzy matching utilities for search
//!
//! Provides fuzzy string matching for library searches,
//! matching Python's difflib-based implementation.

use strsim::normalized_levenshtein;

/// Result of a fuzzy match with the matched value and score
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub value: String,
    pub score: f64,
}

/// Normalize text for better matching
/// Matches Python's `normalize_text()` function
pub fn normalize_text(text: &str) -> String {
    let mut result = text.to_lowercase();

    // Number replacements (spoken -> library format)
    let replacements = [
        ("number one", "no. 1"),
        ("number two", "no. 2"),
        ("number three", "no. 3"),
        ("number four", "no. 4"),
        ("number five", "no. 5"),
        ("number six", "no. 6"),
        ("number seven", "no. 7"),
        ("number eight", "no. 8"),
        ("number nine", "no. 9"),
        ("number 1", "no. 1"),
        ("number 2", "no. 2"),
        ("number 3", "no. 3"),
        ("number 4", "no. 4"),
        ("number 5", "no. 5"),
        ("number 6", "no. 6"),
        ("number 7", "no. 7"),
        ("number 8", "no. 8"),
        ("number 9", "no. 9"),
        (" op ", " op. "),
        (" opus ", " op. "),
        ("simply", "symphony"),
    ];

    for (from, to) in replacements {
        result = result.replace(from, to);
    }

    result
}

/// Strip common articles for better matching
pub fn strip_articles(text: &str) -> String {
    let lower = text.to_lowercase();
    let articles = ["the ", "a ", "an "];

    for article in articles {
        if lower.starts_with(article) {
            return text[article.len()..].to_string();
        }
    }
    text.to_string()
}

/// Find matches in a list of candidates
///
/// Returns up to `n` matches with scores above `cutoff`
/// Matches Python's `find_matches()` function
pub fn find_matches(
    search_term: &str,
    candidates: &[String],
    n: usize,
    cutoff: f64,
) -> Vec<FuzzyMatch> {
    let normalized = normalize_text(search_term);
    let search_lower = normalized.to_lowercase();

    let mut matches: Vec<FuzzyMatch> = Vec::new();

    // 1. Check for exact matches first
    for candidate in candidates {
        if candidate.to_lowercase() == search_lower {
            matches.push(FuzzyMatch {
                value: candidate.clone(),
                score: 1.0,
            });
        }
    }

    // 2. Fuzzy match using normalized_levenshtein
    for candidate in candidates {
        let candidate_lower = candidate.to_lowercase();

        // Skip if already added as exact
        if matches.iter().any(|m| m.value == *candidate) {
            continue;
        }

        let score = normalized_levenshtein(&search_lower, &candidate_lower);

        if score >= cutoff {
            matches.push(FuzzyMatch {
                value: candidate.clone(),
                score,
            });
        }
    }

    // 3. Sort by score descending
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 4. Limit to n results
    matches.truncate(n);

    matches
}

/// Find the best match above a minimum score
///
/// Returns None if no match meets the cutoff
pub fn find_best_match(
    search_term: &str,
    candidates: &[String],
    cutoff: f64,
) -> Option<FuzzyMatch> {
    let matches = find_matches(search_term, candidates, 1, cutoff);
    matches.into_iter().next()
}

/// Calculate similarity score between two strings
pub fn similarity(a: &str, b: &str) -> f64 {
    normalized_levenshtein(&a.to_lowercase(), &b.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_text() {
        assert_eq!(normalize_text("Symphony Number One"), "symphony no. 1");
        // Note: " opus " requires surrounding spaces (matches Python)
        assert_eq!(normalize_text("Sonata opus 27"), "sonata op. 27");
    }

    #[test]
    fn test_find_matches() {
        let candidates = vec![
            "Beethoven".to_string(),
            "Bach".to_string(),
            "Brahms".to_string(),
        ];

        let matches = find_matches("beethoven", &candidates, 5, 0.6);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].value, "Beethoven");
        assert!(matches[0].score >= 0.9);
    }

    #[test]
    fn test_find_best_match() {
        let candidates = vec!["The Beatles".to_string(), "Beach Boys".to_string()];

        let best = find_best_match("beatles", &candidates, 0.6);
        assert!(best.is_some());
        assert_eq!(best.unwrap().value, "The Beatles");
    }

    #[test]
    fn test_play_verb_variations() {
        // Guardrail: Ensure fuzzy matching handles common ASR errors for "play"
        // These values must remain above the threshold used in CommandProcessor (0.6)
        assert!(similarity("play", "play") >= 0.6);
        assert!(similarity("played", "play") >= 0.6); // 0.66
        assert!(similarity("plate", "play") >= 0.6); // 0.6
        assert!(similarity("plays", "play") >= 0.6); // 0.8

        // Ensure distinct words don't match
        assert!(similarity("pause", "play") < 0.5);
        assert!(similarity("stop", "play") < 0.5);
    }
}
