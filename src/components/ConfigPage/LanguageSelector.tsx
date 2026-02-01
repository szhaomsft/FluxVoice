import { useState, useRef, useEffect } from 'react';
import { ChevronDown, Check, X } from 'lucide-react';

// Azure Speech Service Fast Transcription supported locales
// https://learn.microsoft.com/en-us/azure/ai-services/speech-service/fast-transcription-create
const AVAILABLE_LOCALES = [
  // English
  { code: 'en-US', name: 'English', nativeName: 'English (United States)' },
  { code: 'en-GB', name: 'English', nativeName: 'English (United Kingdom)' },
  { code: 'en-AU', name: 'English', nativeName: 'English (Australia)' },
  { code: 'en-CA', name: 'English', nativeName: 'English (Canada)' },
  { code: 'en-IN', name: 'English', nativeName: 'English (India)' },
  { code: 'en-IE', name: 'English', nativeName: 'English (Ireland)' },
  { code: 'en-NZ', name: 'English', nativeName: 'English (New Zealand)' },
  { code: 'en-SG', name: 'English', nativeName: 'English (Singapore)' },
  { code: 'en-ZA', name: 'English', nativeName: 'English (South Africa)' },
  { code: 'en-HK', name: 'English', nativeName: 'English (Hong Kong)' },
  { code: 'en-PH', name: 'English', nativeName: 'English (Philippines)' },
  // Chinese
  { code: 'zh-CN', name: 'Chinese', nativeName: '中文 (简体)' },
  { code: 'zh-HK', name: 'Chinese', nativeName: '中文 (香港)' },
  // Spanish
  { code: 'es-ES', name: 'Spanish', nativeName: 'Español (España)' },
  { code: 'es-MX', name: 'Spanish', nativeName: 'Español (México)' },
  { code: 'es-AR', name: 'Spanish', nativeName: 'Español (Argentina)' },
  { code: 'es-CO', name: 'Spanish', nativeName: 'Español (Colombia)' },
  { code: 'es-CL', name: 'Spanish', nativeName: 'Español (Chile)' },
  // French
  { code: 'fr-FR', name: 'French', nativeName: 'Français (France)' },
  { code: 'fr-CA', name: 'French', nativeName: 'Français (Canada)' },
  // German
  { code: 'de-DE', name: 'German', nativeName: 'Deutsch (Deutschland)' },
  { code: 'de-AT', name: 'German', nativeName: 'Deutsch (Österreich)' },
  { code: 'de-CH', name: 'German', nativeName: 'Deutsch (Schweiz)' },
  // Japanese & Korean
  { code: 'ja-JP', name: 'Japanese', nativeName: '日本語 (日本)' },
  { code: 'ko-KR', name: 'Korean', nativeName: '한국어 (대한민국)' },
  // Portuguese
  { code: 'pt-BR', name: 'Portuguese', nativeName: 'Português (Brasil)' },
  { code: 'pt-PT', name: 'Portuguese', nativeName: 'Português (Portugal)' },
  // Italian & Russian
  { code: 'it-IT', name: 'Italian', nativeName: 'Italiano (Italia)' },
  { code: 'ru-RU', name: 'Russian', nativeName: 'Русский (Россия)' },
  // Arabic
  { code: 'ar-SA', name: 'Arabic', nativeName: 'العربية (السعودية)' },
  { code: 'ar-EG', name: 'Arabic', nativeName: 'العربية (مصر)' },
  { code: 'ar-AE', name: 'Arabic', nativeName: 'العربية (الإمارات)' },
  // South Asian
  { code: 'hi-IN', name: 'Hindi', nativeName: 'हिन्दी (भारत)' },
  { code: 'bn-IN', name: 'Bengali', nativeName: 'বাংলা (ভারত)' },
  { code: 'ta-IN', name: 'Tamil', nativeName: 'தமிழ் (இந்தியா)' },
  { code: 'te-IN', name: 'Telugu', nativeName: 'తెలుగు (భారతదేశం)' },
  { code: 'mr-IN', name: 'Marathi', nativeName: 'मराठी (भारत)' },
  { code: 'gu-IN', name: 'Gujarati', nativeName: 'ગુજરાતી (ભારત)' },
  { code: 'kn-IN', name: 'Kannada', nativeName: 'ಕನ್ನಡ (ಭಾರತ)' },
  { code: 'ml-IN', name: 'Malayalam', nativeName: 'മലയാളം (ഇന്ത്യ)' },
  // Southeast Asian
  { code: 'th-TH', name: 'Thai', nativeName: 'ไทย (ประเทศไทย)' },
  { code: 'vi-VN', name: 'Vietnamese', nativeName: 'Tiếng Việt (Việt Nam)' },
  { code: 'id-ID', name: 'Indonesian', nativeName: 'Bahasa Indonesia (Indonesia)' },
  { code: 'ms-MY', name: 'Malay', nativeName: 'Bahasa Melayu (Malaysia)' },
  { code: 'fil-PH', name: 'Filipino', nativeName: 'Filipino (Pilipinas)' },
  // European
  { code: 'tr-TR', name: 'Turkish', nativeName: 'Türkçe (Türkiye)' },
  { code: 'pl-PL', name: 'Polish', nativeName: 'Polski (Polska)' },
  { code: 'nl-NL', name: 'Dutch', nativeName: 'Nederlands (Nederland)' },
  { code: 'sv-SE', name: 'Swedish', nativeName: 'Svenska (Sverige)' },
  { code: 'da-DK', name: 'Danish', nativeName: 'Dansk (Danmark)' },
  { code: 'fi-FI', name: 'Finnish', nativeName: 'Suomi (Suomi)' },
  { code: 'nb-NO', name: 'Norwegian', nativeName: 'Norsk Bokmål (Norge)' },
  { code: 'cs-CZ', name: 'Czech', nativeName: 'Čeština (Česká republika)' },
  { code: 'hu-HU', name: 'Hungarian', nativeName: 'Magyar (Magyarország)' },
  { code: 'ro-RO', name: 'Romanian', nativeName: 'Română (România)' },
  { code: 'el-GR', name: 'Greek', nativeName: 'Ελληνικά (Ελλάδα)' },
  { code: 'bg-BG', name: 'Bulgarian', nativeName: 'Български (България)' },
  { code: 'hr-HR', name: 'Croatian', nativeName: 'Hrvatski (Hrvatska)' },
  { code: 'sk-SK', name: 'Slovak', nativeName: 'Slovenčina (Slovensko)' },
  { code: 'sl-SI', name: 'Slovenian', nativeName: 'Slovenščina (Slovenija)' },
  { code: 'et-EE', name: 'Estonian', nativeName: 'Eesti (Eesti)' },
  { code: 'lv-LV', name: 'Latvian', nativeName: 'Latviešu (Latvija)' },
  { code: 'lt-LT', name: 'Lithuanian', nativeName: 'Lietuvių (Lietuva)' },
  { code: 'uk-UA', name: 'Ukrainian', nativeName: 'Українська (Україна)' },
  // Other
  { code: 'he-IL', name: 'Hebrew', nativeName: 'עברית (ישראל)' },
  { code: 'fa-IR', name: 'Persian', nativeName: 'فارسی (ایران)' },
  { code: 'af-ZA', name: 'Afrikaans', nativeName: 'Afrikaans (Suid-Afrika)' },
  { code: 'ca-ES', name: 'Catalan', nativeName: 'Català (Espanya)' },
];

interface LanguageSelectorProps {
  selectedLanguages: string[];
  onChange: (languages: string[]) => void;
}

export const LanguageSelector: React.FC<LanguageSelectorProps> = ({
  selectedLanguages,
  onChange,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const toggleLanguage = (code: string) => {
    if (selectedLanguages.includes(code)) {
      // Don't allow removing the last language
      if (selectedLanguages.length > 1) {
        onChange(selectedLanguages.filter((l) => l !== code));
      }
    } else {
      onChange([...selectedLanguages, code]);
    }
  };

  const removeLanguage = (code: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (selectedLanguages.length > 1) {
      onChange(selectedLanguages.filter((l) => l !== code));
    }
  };

  const getLanguageName = (code: string) => {
    const locale = AVAILABLE_LOCALES.find((l) => l.code === code);
    return locale?.nativeName || code;
  };

  return (
    <div className="relative" ref={dropdownRef}>
      <div
        onClick={() => setIsOpen(!isOpen)}
        className="min-h-[42px] px-3 py-2 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg cursor-pointer flex items-center justify-between gap-2 hover:border-amber-500 dark:hover:border-amber-400 transition-colors"
      >
        <div className="flex flex-wrap gap-1.5 flex-1">
          {selectedLanguages.length === 0 ? (
            <span className="text-gray-400">Select languages...</span>
          ) : (
            selectedLanguages.map((code) => (
              <span
                key={code}
                className="inline-flex items-center gap-1 px-2 py-0.5 bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 rounded text-sm"
              >
                {getLanguageName(code)}
                {selectedLanguages.length > 1 && (
                  <button
                    onClick={(e) => removeLanguage(code, e)}
                    className="hover:text-amber-900 dark:hover:text-amber-100"
                  >
                    <X className="w-3 h-3" />
                  </button>
                )}
              </span>
            ))
          )}
        </div>
        <ChevronDown
          className={`w-5 h-5 text-gray-400 transition-transform ${isOpen ? 'rotate-180' : ''}`}
        />
      </div>

      {isOpen && (
        <div className="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg max-h-64 overflow-y-auto">
          {AVAILABLE_LOCALES.map((lang) => (
            <div
              key={lang.code}
              onClick={() => toggleLanguage(lang.code)}
              className={`px-3 py-2 cursor-pointer flex items-center justify-between hover:bg-gray-100 dark:hover:bg-gray-700 ${
                selectedLanguages.includes(lang.code)
                  ? 'bg-amber-50 dark:bg-amber-900/20'
                  : ''
              }`}
            >
              <span className="text-gray-900 dark:text-gray-100">
                {lang.nativeName}
                <span className="text-gray-400 dark:text-gray-500 text-sm ml-2">
                  ({lang.code})
                </span>
              </span>
              {selectedLanguages.includes(lang.code) && (
                <Check className="w-4 h-4 text-amber-500" />
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
