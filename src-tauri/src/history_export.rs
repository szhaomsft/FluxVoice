use crate::audio::format::AudioFormat;
use crate::commands::TranscriptionHistoryItem;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct HistoryExportResult {
    pub directory: String,
    pub transcription_count: usize,
    pub audio_count: usize,
    pub missing_audio_count: usize,
}

#[derive(Serialize)]
struct ExportEntry {
    timestamp: u64,
    recorded_at_utc: String,
    original: String,
    polished: Option<String>,
    final_text: String,
    audio_file: Option<String>,
    transcript_file: String,
}

#[derive(Serialize)]
struct ExportManifest {
    schema_version: u32,
    exported_at_utc: String,
    recordings: Vec<ExportEntry>,
}

fn create_export_directory(parent: &Path, now: DateTime<Utc>) -> Result<PathBuf, String> {
    let metadata = fs::metadata(parent).map_err(|error| {
        format!(
            "Cannot access export directory {}: {error}",
            parent.display()
        )
    })?;
    if !metadata.is_dir() {
        return Err("The export destination must be a directory".into());
    }
    let base = format!("fluxvoice-export-{}", now.format("%Y%m%dT%H%M%S%.3fZ"));
    for suffix in 0..1000 {
        let name = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        let directory = parent.join(name);
        match fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Cannot create export folder {}: {error}",
                    directory.display()
                ))
            }
        }
    }
    Err("Cannot create a unique export folder; choose another directory".into())
}

fn write_new_file(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
    file.write_all(data)
        .map_err(|error| format!("Cannot write {}: {error}", path.display()))
}

pub fn export_all(
    parent: &Path,
    items: &[TranscriptionHistoryItem],
) -> Result<HistoryExportResult, String> {
    if items.is_empty() {
        return Err("No saved transcription history to export".into());
    }
    let now = Utc::now();
    let mut ordered: Vec<_> = items.iter().collect();
    ordered.sort_by_key(|item| item.timestamp);
    let mut recordings = Vec::with_capacity(items.len());
    for (index, item) in ordered.iter().enumerate() {
        let timestamp = i64::try_from(item.timestamp)
            .ok()
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .ok_or_else(|| format!("Recording {} has an invalid timestamp", index + 1))?;
        let base = format!(
            "{:06}_{}",
            index + 1,
            timestamp.format("%Y%m%dT%H%M%S%.3fZ")
        );
        let audio_file = item
            .audio_data
            .as_deref()
            .filter(|audio| !audio.is_empty())
            .map(|audio| {
                AudioFormat::detect(audio).map(|format| format!("{base}.{}", format.extension()))
            })
            .transpose()
            .map_err(|error| format!("Recording {} cannot be exported: {error}", index + 1))?;
        recordings.push(ExportEntry {
            timestamp: item.timestamp,
            recorded_at_utc: timestamp.to_rfc3339_opts(SecondsFormat::Millis, true),
            original: item.original.clone(),
            polished: item.polished.clone(),
            final_text: item.final_text.clone(),
            audio_file,
            transcript_file: format!("{base}.txt"),
        });
    }
    let manifest = ExportManifest {
        schema_version: 1,
        exported_at_utc: now.to_rfc3339_opts(SecondsFormat::Millis, true),
        recordings,
    };
    let metadata = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("Cannot serialize export metadata: {error}"))?;
    let directory = create_export_directory(parent, now)?;
    let write_files = || -> Result<(), String> {
        for (item, entry) in ordered.iter().zip(&manifest.recordings) {
            if let (Some(audio), Some(filename)) = (&item.audio_data, &entry.audio_file) {
                write_new_file(&directory.join(filename), audio)?;
            }
            let mut transcript = format!(
                "Timestamp (UTC): {}\n\nOriginal:\n{}\n",
                entry.recorded_at_utc, entry.original
            );
            if let Some(polished) = &entry.polished {
                transcript.push_str(&format!("\nProcessed:\n{polished}\n"));
            }
            transcript.push_str(&format!("\nFinal:\n{}\n", entry.final_text));
            write_new_file(
                &directory.join(&entry.transcript_file),
                transcript.as_bytes(),
            )?;
        }
        write_new_file(&directory.join("transcripts.json"), &metadata)
    };
    write_files().map_err(|error| {
        format!(
            "Export failed; partial files may remain in {}: {error}",
            directory.display()
        )
    })?;
    let audio_count = manifest
        .recordings
        .iter()
        .filter(|entry| entry.audio_file.is_some())
        .count();
    Ok(HistoryExportResult {
        directory: directory.to_string_lossy().into_owned(),
        transcription_count: items.len(),
        audio_count,
        missing_audio_count: items.len() - audio_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let directory = std::env::temp_dir().join(format!(
                "fluxvoice-export-test-{}-{}-{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            Self(directory)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_dir_all(&self.0) {
                eprintln!(
                    "Could not clean up export test directory {}: {error}",
                    self.0.display()
                );
            }
        }
    }
    fn item(timestamp: u64, audio_data: Option<Vec<u8>>) -> TranscriptionHistoryItem {
        TranscriptionHistoryItem {
            timestamp,
            audio_data,
            original: "你好 Rehaan\nsecond line".into(),
            polished: Some("Processed text".into()),
            final_text: "Final text".into(),
        }
    }

    #[test]
    fn exports_all_formats_and_text_variants_with_unique_utc_names() {
        let parent = TestDirectory::new();
        let timestamp = u64::try_from(
            DateTime::parse_from_rfc3339("2026-10-09T08:28:46.920Z")
                .unwrap()
                .timestamp_millis(),
        )
        .unwrap();
        let audio = [
            b"RIFF\0\0\0\0WAVEpcm".to_vec(),
            b"OggSopus".to_vec(),
            b"ID3\x04\0\0\0\0\0\0mp3".to_vec(),
            vec![0xff, 0xf3, 0x68, 0xc4],
        ];
        let items: Vec<_> = audio
            .iter()
            .map(|data| item(timestamp, Some(data.clone())))
            .collect();
        let result = export_all(&parent.0, &items).unwrap();
        assert_eq!(
            (
                result.transcription_count,
                result.audio_count,
                result.missing_audio_count
            ),
            (4, 4, 0)
        );
        let directory = Path::new(&result.directory);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("transcripts.json")).unwrap()).unwrap();
        for (index, extension) in ["wav", "ogg", "mp3", "mp3"].iter().enumerate() {
            let record = &manifest["recordings"][index];
            let expected = format!("{:06}_20261009T082846.920Z.{extension}", index + 1);
            assert_eq!(record["audio_file"], expected);
            assert_eq!(fs::read(directory.join(&expected)).unwrap(), audio[index]);
            assert_eq!(record["recorded_at_utc"], "2026-10-09T08:28:46.920Z");
            assert_eq!(record["original"], items[index].original);
            assert_eq!(record["polished"], "Processed text");
            assert_eq!(record["final_text"], "Final text");
            let text =
                fs::read_to_string(directory.join(record["transcript_file"].as_str().unwrap()))
                    .unwrap();
            assert!(text.contains("你好 Rehaan\nsecond line"));
            assert!(text.contains("Processed:\nProcessed text"));
            assert!(text.contains("Final:\nFinal text"));
            assert!(record.get("audio_data").is_none());
        }
        assert_eq!(fs::read_dir(directory).unwrap().count(), 9);
    }

    #[test]
    fn preserves_transcripts_when_audio_is_missing_or_empty() {
        let parent = TestDirectory::new();
        let mut first = item(2, None);
        first.polished = None;
        let result = export_all(&parent.0, &[first, item(1, Some(Vec::new()))]).unwrap();
        assert_eq!(
            (
                result.transcription_count,
                result.audio_count,
                result.missing_audio_count
            ),
            (2, 0, 2)
        );
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(Path::new(&result.directory).join("transcripts.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["recordings"][0]["timestamp"], 1);
        assert!(manifest["recordings"][0]["audio_file"].is_null());
        assert!(manifest["recordings"][1]["polished"].is_null());
        assert_eq!(fs::read_dir(&result.directory).unwrap().count(), 3);
    }

    #[test]
    fn rejects_invalid_data_before_creating_any_export_files() {
        let parent = TestDirectory::new();
        assert!(export_all(&parent.0, &[]).unwrap_err().contains("No saved"));
        assert!(export_all(&parent.0, &[item(0, Some(b"unknown".to_vec()))])
            .unwrap_err()
            .contains("Unsupported recording"));
        assert!(export_all(&parent.0, &[item(u64::MAX, None)])
            .unwrap_err()
            .contains("invalid timestamp"));
        assert_eq!(fs::read_dir(&parent.0).unwrap().count(), 0);
    }

    #[test]
    fn never_overwrites_existing_directories_or_files() {
        let parent = TestDirectory::new();
        let now = Utc::now();
        let first = create_export_directory(&parent.0, now).unwrap();
        let protected = first.join("keep.txt");
        fs::write(&protected, b"keep").unwrap();
        let second = create_export_directory(&parent.0, now).unwrap();
        assert_ne!(first, second);
        assert!(write_new_file(&protected, b"replace").is_err());
        assert_eq!(fs::read(&protected).unwrap(), b"keep");
    }

    #[test]
    fn reports_unavailable_or_non_directory_destinations() {
        let parent = TestDirectory::new();
        assert!(export_all(&parent.0.join("missing"), &[item(0, None)]).is_err());
        let file = parent.0.join("file.txt");
        fs::write(&file, b"keep").unwrap();
        assert!(export_all(&file, &[item(0, None)])
            .unwrap_err()
            .contains("must be a directory"));
        assert_eq!(fs::read(file).unwrap(), b"keep");
    }
}
