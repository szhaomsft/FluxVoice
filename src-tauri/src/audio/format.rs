#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioFormat {
    Wav,
    Ogg,
    Mp3,
}

impl AudioFormat {
    pub fn detect(data: &[u8]) -> Result<Self, String> {
        if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE") {
            Ok(Self::Wav)
        } else if data.starts_with(b"OggS") {
            Ok(Self::Ogg)
        } else if (data.starts_with(b"ID3") && data.len() >= 10)
            || data.get(..4).is_some_and(|header| {
                header[0] == 0xff
                    && header[1] & 0xe0 == 0xe0
                    && header[1] & 0x06 == 0x02
                    && header[1] & 0x18 != 0x08
                    && header[2] & 0xf0 != 0
                    && header[2] & 0xf0 != 0xf0
                    && header[2] & 0x0c != 0x0c
            })
        {
            Ok(Self::Mp3)
        } else {
            Err("Unsupported recording format: expected Opus/OGG, MP3, or WAV audio.".into())
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Ogg => "ogg",
            Self::Mp3 => "mp3",
        }
    }

    pub fn upload_metadata(self) -> (&'static str, &'static str) {
        match self {
            Self::Wav => ("audio.wav", "audio/wav"),
            Self::Ogg => ("audio.ogg", "audio/ogg"),
            Self::Mp3 => ("audio.mp3", "audio/mpeg"),
        }
    }
}
