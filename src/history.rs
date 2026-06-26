use crate::{
    cache::{RecentFilter, RecentQuery},
    task::TaskType,
};

const SEPARATOR: &str = "------------------------------------------------------------";

pub fn format_recent_queries(filter: RecentFilter, queries: &[RecentQuery]) -> String {
    let mut output = String::new();

    output.push_str("RECENT QUERIES\n");
    output.push_str(&format!("FILTER: {}\n", filter_label(filter)));
    output.push_str(&format!("COUNT: {}\n", queries.len()));

    if queries.is_empty() {
        output.push_str("\nNo recent queries found.");
        return output;
    }

    for (index, query) in queries.iter().enumerate() {
        output.push_str("\n\n");
        if index > 0 {
            output.push_str(SEPARATOR);
            output.push_str("\n\n");
        }

        output.push_str(&format!(
            "[{}] {}\n",
            index + 1,
            task_label(query.task_type)
        ));
        output.push_str(&format!("TIME: {}\n", query.last_used_at));
        output.push_str(&format!("TARGET: {}\n", query.target_lang));
        output.push_str(&format!("MODEL: {}/{}\n", query.provider, query.model));
        output.push_str("INPUT:\n");
        output.push_str(&indent_block(&query.input));
        output.push('\n');
        output.push_str("OUTPUT:\n");
        output.push_str(&indent_block(&query.output));
    }

    output
}

fn filter_label(filter: RecentFilter) -> &'static str {
    match filter {
        RecentFilter::All => "all",
        RecentFilter::Words => "words",
        RecentFilter::Sentences => "sentences",
    }
}

fn task_label(task_type: TaskType) -> &'static str {
    match task_type {
        TaskType::WordLookup => "word",
        TaskType::PhraseLookup => "phrase",
        TaskType::SentenceTranslation => "sentence",
    }
}

fn indent_block(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_recent_query_output() {
        let output = format_recent_queries(
            RecentFilter::All,
            &[RecentQuery {
                input: "hello".into(),
                task_type: TaskType::SentenceTranslation,
                target_lang: "auto".into(),
                provider: "deepseek".into(),
                model: "deepseek-v4-flash".into(),
                output: "你好".into(),
                created_at: 1,
                last_used_at: 2,
            }],
        );

        assert!(output.contains("RECENT QUERIES"));
        assert!(output.contains("FILTER: all"));
        assert!(output.contains("[1] sentence"));
        assert!(output.contains("INPUT:\n  hello"));
        assert!(output.contains("OUTPUT:\n  你好"));
    }
}
