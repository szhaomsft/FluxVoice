use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

#[cfg(windows)]
mod windows;

const MAX_PHRASES: usize = 100;
const MAX_PHRASE_CHARS: usize = 80;
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct Capture {
    receiver: oneshot::Receiver<Result<Vec<String>, String>>,
    started: Instant,
}

impl Capture {
    pub async fn finish(self) -> Result<Vec<String>, String> {
        let remaining = CAPTURE_TIMEOUT.saturating_sub(self.started.elapsed());
        match tokio::time::timeout(remaining, self.receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Screen context worker stopped unexpectedly".into()),
            Err(_) => Err("Screen context capture timed out".into()),
        }
    }
}

pub fn start_capture() -> Result<Capture, String> {
    #[cfg(windows)]
    {
        windows::start_capture()
    }
    #[cfg(not(windows))]
    {
        Err("Screen phrase hints are only available on Windows".into())
    }
}

#[derive(Default)]
struct PhraseCollector {
    phrases: Vec<String>,
    seen: HashSet<String>,
}

impl PhraseCollector {
    fn add(&mut self, phrase: &str) {
        let phrase = phrase.trim();
        let count = phrase.chars().count();
        if self.phrases.len() < MAX_PHRASES
            && (3..=MAX_PHRASE_CHARS).contains(&count)
            && phrase.chars().any(char::is_alphabetic)
            && self.seen.insert(phrase.to_lowercase())
        {
            self.phrases.push(phrase.to_string());
        }
    }

    fn collect(&mut self, text: &str) {
        const COMMON_WORDS: &[&str] = &[
            "the", "and", "for", "with", "this", "that", "from", "are", "was", "you", "your",
            "have", "not", "but", "can", "all", "file", "edit", "view", "help", "window", "close",
            "minimize", "maximize",
        ];
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if (2..=6).contains(&words.len()) {
                self.add(&words.join(" "));
            }
            for word in
                line.split(|c: char| !c.is_alphanumeric() && !matches!(c, '-' | '_' | '\'' | '.'))
            {
                let word = word.trim_matches(|c: char| !c.is_alphanumeric());
                if !COMMON_WORDS.contains(&word.to_lowercase().as_str()) {
                    self.add(word);
                }
                if self.phrases.len() == MAX_PHRASES {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_names_and_terms_without_case_duplicates() {
        let mut collector = PhraseCollector::default();
        collector.collect(
            "Rehaan Kumatani\nFluxVoice fluxvoice\nUIAutomation speech-to-text\nthe and 123",
        );
        assert!(collector.phrases.contains(&"Rehaan Kumatani".into()));
        assert!(collector.phrases.contains(&"FluxVoice".into()));
        assert!(collector.phrases.contains(&"UIAutomation".into()));
        assert!(collector.phrases.contains(&"speech-to-text".into()));
        assert!(!collector.phrases.contains(&"fluxvoice".into()));
        assert!(!collector.phrases.contains(&"the".into()));
        assert!(!collector.phrases.contains(&"123".into()));
    }

    #[test]
    fn bounds_phrase_count_and_unicode_length() {
        let mut collector = PhraseCollector::default();
        collector.collect(&"界".repeat(MAX_PHRASE_CHARS + 1));
        assert!(collector.phrases.is_empty());
        for index in 0..MAX_PHRASES + 20 {
            collector.collect(&format!("Term{index}"));
        }
        assert_eq!(collector.phrases.len(), MAX_PHRASES);
        assert!(collector
            .phrases
            .iter()
            .all(|phrase| phrase.chars().count() <= MAX_PHRASE_CHARS));
    }

    #[tokio::test]
    async fn capture_surfaces_worker_failure_and_timeout() {
        let (sender, receiver) = oneshot::channel();
        drop(sender);
        assert!(Capture {
            receiver,
            started: Instant::now()
        }
        .finish()
        .await
        .unwrap_err()
        .contains("stopped"));
        let (_sender, receiver) = oneshot::channel();
        assert!(Capture {
            receiver,
            started: Instant::now() - CAPTURE_TIMEOUT,
        }
        .finish()
        .await
        .unwrap_err()
        .contains("timed out"));
    }
}
