// IndexedDB storage for audio data (too large for localStorage)

export function getAudioFormat(audioData: readonly number[]): { extension: 'wav' | 'ogg' | 'mp3'; mimeType: string } {
  const header = String.fromCharCode(...audioData.slice(0, 4));
  if (header === 'RIFF' && String.fromCharCode(...audioData.slice(8, 12)) === 'WAVE') {
    return { extension: 'wav', mimeType: 'audio/wav' };
  }
  if (header === 'OggS') {
    return { extension: 'ogg', mimeType: 'audio/ogg; codecs=opus' };
  }
  if ((header.startsWith('ID3') && audioData.length >= 10)
    || (audioData.length >= 4 && audioData[0] === 0xff && (audioData[1] & 0xe0) === 0xe0
      && (audioData[1] & 0x06) === 0x02 && (audioData[1] & 0x18) !== 0x08
      && (audioData[2] & 0xf0) !== 0 && (audioData[2] & 0xf0) !== 0xf0
      && (audioData[2] & 0x0c) !== 0x0c)) {
    return { extension: 'mp3', mimeType: 'audio/mpeg' };
  }
  throw new Error('Unsupported recording format: expected WAV, MP3, or Opus/OGG audio.');
}

const DB_NAME = 'fluxvoice_audio_db';
const DB_VERSION = 1;
const STORE_NAME = 'audio_recordings';

let dbPromise: Promise<IDBDatabase> | null = null;

function openDB(): Promise<IDBDatabase> {
  if (dbPromise) return dbPromise;

  dbPromise = new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onerror = () => {
      console.error('Failed to open IndexedDB:', request.error);
      reject(request.error);
    };

    request.onsuccess = () => {
      resolve(request.result);
    };

    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'timestamp' });
      }
    };
  });

  return dbPromise;
}

export async function saveAudioData(timestamp: number, audioData: number[]): Promise<void> {
  try {
    const db = await openDB();
    const transaction = db.transaction(STORE_NAME, 'readwrite');
    const store = transaction.objectStore(STORE_NAME);

    return new Promise((resolve, reject) => {
      const request = store.put({ timestamp, audioData });
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  } catch (err) {
    console.error('Failed to save audio data:', err);
  }
}

export async function getAudioData(timestamp: number): Promise<number[] | null> {
  try {
    const db = await openDB();
    const transaction = db.transaction(STORE_NAME, 'readonly');
    const store = transaction.objectStore(STORE_NAME);

    return new Promise((resolve, reject) => {
      const request = store.get(timestamp);
      request.onsuccess = () => {
        const result = request.result;
        resolve(result ? result.audioData : null);
      };
      request.onerror = () => reject(request.error);
    });
  } catch (err) {
    console.error('Failed to get audio data:', err);
    return null;
  }
}

export async function deleteAudioData(timestamp: number): Promise<void> {
  try {
    const db = await openDB();
    const transaction = db.transaction(STORE_NAME, 'readwrite');
    const store = transaction.objectStore(STORE_NAME);

    return new Promise((resolve, reject) => {
      const request = store.delete(timestamp);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  } catch (err) {
    console.error('Failed to delete audio data:', err);
  }
}

export async function clearAllAudioData(): Promise<void> {
  try {
    const db = await openDB();
    const transaction = db.transaction(STORE_NAME, 'readwrite');
    const store = transaction.objectStore(STORE_NAME);

    return new Promise((resolve, reject) => {
      const request = store.clear();
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  } catch (err) {
    console.error('Failed to clear audio data:', err);
  }
}
