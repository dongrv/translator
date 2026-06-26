#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    WordLookup,
    PhraseLookup,
    SentenceTranslation,
}

impl TaskType {
    pub fn detect(input: &str) -> Self {
        let text = input.trim();
        if !is_english_lookup_candidate(text) {
            return Self::SentenceTranslation;
        }

        let tokens = text.split_whitespace().count();
        if tokens == 1 {
            Self::WordLookup
        } else if tokens <= 5 && !has_sentence_punctuation(text) {
            Self::PhraseLookup
        } else {
            Self::SentenceTranslation
        }
    }

    pub fn as_prompt_label(self) -> &'static str {
        match self {
            Self::WordLookup => "word lookup",
            Self::PhraseLookup => "phrase lookup",
            Self::SentenceTranslation => "sentence translation",
        }
    }

    pub fn cache_label(self) -> &'static str {
        match self {
            Self::WordLookup => "word",
            Self::PhraseLookup => "phrase",
            Self::SentenceTranslation => "sentence",
        }
    }
}

fn is_english_lookup_candidate(text: &str) -> bool {
    let mut has_letter = false;

    for ch in text.chars() {
        if ch.is_ascii_alphabetic() {
            has_letter = true;
        } else if ch.is_ascii_whitespace() || ch == '-' || ch == '\'' || ch == '/' || ch == '.' {
            continue;
        } else {
            return false;
        }
    }

    has_letter
}

fn has_sentence_punctuation(text: &str) -> bool {
    text.contains('?')
        || text.contains('!')
        || text.contains('。')
        || text.contains('？')
        || text.contains('！')
        || text.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_word_lookup() {
        assert_eq!(TaskType::detect("burst"), TaskType::WordLookup);
    }

    #[test]
    fn detects_phrase_lookup() {
        assert_eq!(TaskType::detect("traffic burst"), TaskType::PhraseLookup);
    }

    #[test]
    fn detects_sentence_translation() {
        assert_eq!(
            TaskType::detect("The server is down."),
            TaskType::SentenceTranslation
        );
        assert_eq!(
            TaskType::detect("AI will reshape the industry."),
            TaskType::SentenceTranslation
        );
    }
}
