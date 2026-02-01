import { useState, useRef, useEffect } from 'react';
import { ChevronDown, Check, X } from 'lucide-react';

// Azure Speech Service supported languages
// https://learn.microsoft.com/en-us/azure/ai-services/speech-service/language-support
const AVAILABLE_LANGUAGES = [
  { code: 'en-US', name: 'English (US)' },
  { code: 'en-GB', name: 'English (UK)' },
  { code: 'en-AU', name: 'English (Australia)' },
  { code: 'zh-CN', name: '中文 (简体)' },
  { code: 'zh-TW', name: '中文 (繁體)' },
  { code: 'zh-HK', name: '中文 (香港)' },
  { code: 'ja-JP', name: '日本語' },
  { code: 'ko-KR', name: '한국어' },
  { code: 'es-ES', name: 'Español (España)' },
  { code: 'es-MX', name: 'Español (México)' },
  { code: 'fr-FR', name: 'Français (France)' },
  { code: 'fr-CA', name: 'Français (Canada)' },
  { code: 'de-DE', name: 'Deutsch' },
  { code: 'it-IT', name: 'Italiano' },
  { code: 'pt-BR', name: 'Português (Brasil)' },
  { code: 'pt-PT', name: 'Português (Portugal)' },
  { code: 'ru-RU', name: 'Русский' },
  { code: 'ar-SA', name: 'العربية' },
  { code: 'hi-IN', name: 'हिन्दी' },
  { code: 'th-TH', name: 'ไทย' },
  { code: 'vi-VN', name: 'Tiếng Việt' },
  { code: 'nl-NL', name: 'Nederlands' },
  { code: 'pl-PL', name: 'Polski' },
  { code: 'tr-TR', name: 'Türkçe' },
  { code: 'sv-SE', name: 'Svenska' },
  { code: 'da-DK', name: 'Dansk' },
  { code: 'nb-NO', name: 'Norsk' },
  { code: 'fi-FI', name: 'Suomi' },
  { code: 'cs-CZ', name: 'Čeština' },
  { code: 'el-GR', name: 'Ελληνικά' },
  { code: 'he-IL', name: 'עברית' },
  { code: 'id-ID', name: 'Bahasa Indonesia' },
  { code: 'ms-MY', name: 'Bahasa Melayu' },
  { code: 'uk-UA', name: 'Українська' },
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
    return AVAILABLE_LANGUAGES.find((l) => l.code === code)?.name || code;
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
          {AVAILABLE_LANGUAGES.map((lang) => (
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
                {lang.name}
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
