import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { BarChart3, Clock, Type, Mic } from 'lucide-react';

interface DailyStats {
  date: string;
  transcription_count: number;
  total_characters: number;
  total_duration_secs: number;
}

interface Stats {
  total_transcriptions: number;
  total_characters: number;
  total_duration_secs: number;
  daily_stats: DailyStats[];
}

function formatDuration(secs: number): string {
  if (secs < 60) {
    return `${Math.round(secs)}s`;
  } else if (secs < 3600) {
    const mins = Math.floor(secs / 60);
    const remainingSecs = Math.round(secs % 60);
    return `${mins}m ${remainingSecs}s`;
  } else {
    const hours = Math.floor(secs / 3600);
    const mins = Math.floor((secs % 3600) / 60);
    return `${hours}h ${mins}m`;
  }
}

function formatDate(dateStr: string): string {
  // dateStr is YYYY-MM-DD
  const parts = dateStr.split('-');
  if (parts.length === 3) {
    return `${parts[1]}/${parts[2]}`;
  }
  return dateStr;
}

export const UsageStats: React.FC = () => {
  const [stats, setStats] = useState<Stats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const loadStats = async () => {
      try {
        const data = await invoke<Stats>('get_stats');
        setStats(data);
      } catch (err) {
        console.error('Failed to load stats:', err);
      } finally {
        setLoading(false);
      }
    };

    loadStats();
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="text-gray-500 dark:text-gray-400">Loading stats...</div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="text-gray-500 dark:text-gray-400">Failed to load stats</div>
      </div>
    );
  }

  // Get last 7 days for the chart
  const last7Days = stats.daily_stats.slice(-7);
  const maxChars = Math.max(...last7Days.map(d => d.total_characters), 1);

  return (
    <div className="space-y-6">
      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-blue-100 dark:bg-blue-900/30 rounded-full">
              <Mic className="w-6 h-6 text-blue-600 dark:text-blue-400" />
            </div>
            <div>
              <p className="text-sm text-gray-500 dark:text-gray-400">Total Transcriptions</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {stats.total_transcriptions.toLocaleString()}
              </p>
            </div>
          </div>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-green-100 dark:bg-green-900/30 rounded-full">
              <Type className="w-6 h-6 text-green-600 dark:text-green-400" />
            </div>
            <div>
              <p className="text-sm text-gray-500 dark:text-gray-400">Total Characters</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {stats.total_characters.toLocaleString()}
              </p>
            </div>
          </div>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-purple-100 dark:bg-purple-900/30 rounded-full">
              <Clock className="w-6 h-6 text-purple-600 dark:text-purple-400" />
            </div>
            <div>
              <p className="text-sm text-gray-500 dark:text-gray-400">Total Recording Time</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {formatDuration(stats.total_duration_secs)}
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Daily Chart */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center gap-2 mb-4">
          <BarChart3 className="w-5 h-5 text-gray-500 dark:text-gray-400" />
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white">Last 7 Days</h3>
        </div>

        {last7Days.length === 0 ? (
          <div className="text-center py-8 text-gray-500 dark:text-gray-400">
            No data yet. Start transcribing to see your stats!
          </div>
        ) : (
          <div className="space-y-3">
            {last7Days.map((day) => (
              <div key={day.date} className="flex items-center gap-4">
                <div className="w-16 text-sm text-gray-500 dark:text-gray-400">
                  {formatDate(day.date)}
                </div>
                <div className="flex-1">
                  <div className="h-6 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-gradient-to-r from-blue-500 to-purple-500 rounded-full transition-all duration-300"
                      style={{ width: `${(day.total_characters / maxChars) * 100}%` }}
                    />
                  </div>
                </div>
                <div className="w-24 text-right text-sm text-gray-600 dark:text-gray-300">
                  {day.total_characters.toLocaleString()} chars
                </div>
                <div className="w-16 text-right text-sm text-gray-500 dark:text-gray-400">
                  {day.transcription_count}x
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Today's Stats */}
      {stats.daily_stats.length > 0 && (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">Today</h3>
          {(() => {
            const today = stats.daily_stats[stats.daily_stats.length - 1];
            return (
              <div className="grid grid-cols-3 gap-4 text-center">
                <div>
                  <p className="text-3xl font-bold text-blue-600 dark:text-blue-400">
                    {today.transcription_count}
                  </p>
                  <p className="text-sm text-gray-500 dark:text-gray-400">Transcriptions</p>
                </div>
                <div>
                  <p className="text-3xl font-bold text-green-600 dark:text-green-400">
                    {today.total_characters.toLocaleString()}
                  </p>
                  <p className="text-sm text-gray-500 dark:text-gray-400">Characters</p>
                </div>
                <div>
                  <p className="text-3xl font-bold text-purple-600 dark:text-purple-400">
                    {formatDuration(today.total_duration_secs)}
                  </p>
                  <p className="text-sm text-gray-500 dark:text-gray-400">Recording Time</p>
                </div>
              </div>
            );
          })()}
        </div>
      )}
    </div>
  );
};
