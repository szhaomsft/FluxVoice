export interface TranscriptionResult {
  text: string;
  polished?: string;
}

export interface AudioLevelUpdate {
  level: number;
  timestamp: number;
}
