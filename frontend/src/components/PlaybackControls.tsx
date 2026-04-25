import { useCallback, useEffect, useRef } from 'react';
import { useAppStore } from '../stores/useAppStore';

const SPEEDS = [0.5, 1, 2, 5, 10, 20];

interface PlaybackControlsProps {
  onControl?: (action: 'play' | 'pause' | 'step_forward' | 'step_backward') => void;
  onSpeedChange?: (speed: number) => void;
  onSeek?: (index: number) => void;
}

export default function PlaybackControls({ onControl, onSpeedChange, onSeek }: PlaybackControlsProps) {
  const playback = useAppStore((s) => s.playback);
  const setPlayback = useAppStore((s) => s.setPlayback);
  const speedRef = useRef<HTMLSelectElement>(null);

  const handlePlayPause = useCallback(() => {
    if (playback.status === 'playing') {
      setPlayback({ status: 'paused' });
      onControl?.('pause');
    } else {
      setPlayback({ status: 'playing' });
      onControl?.('play');
    }
  }, [playback.status, setPlayback, onControl]);

  const handleStepForward = useCallback(() => {
    setPlayback({ status: 'stepping' });
    onControl?.('step_forward');
  }, [setPlayback, onControl]);

  const handleStepBackward = useCallback(() => {
    setPlayback({ status: 'stepping' });
    onControl?.('step_backward');
  }, [setPlayback, onControl]);

  const handleSpeedChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const speed = parseFloat(e.target.value);
      setPlayback({ speed });
      onSpeedChange?.(speed);
    },
    [setPlayback, onSpeedChange]
  );

  const handleProgressChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const index = parseInt(e.target.value, 10);
      setPlayback({ currentBarIndex: index });
      onSeek?.(index);
    },
    [setPlayback, onSeek]
  );

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === ' ') {
        e.preventDefault();
        handlePlayPause();
      }
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        handleStepForward();
      }
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        handleStepBackward();
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [handlePlayPause, handleStepForward, handleStepBackward]);

  const isPlaying = playback.status === 'playing';

  return (
    <div className="flex items-center gap-3 px-4 py-2 bg-slate-900 border-t border-slate-800 text-slate-200 select-none">
      <button
        onClick={handleStepBackward}
        className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-sm font-medium transition-colors"
        title="Step Backward (←)"
      >
        ◀ Step
      </button>
      <button
        onClick={handlePlayPause}
        className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
          isPlaying
            ? 'bg-amber-600 hover:bg-amber-500 text-white'
            : 'bg-blue-600 hover:bg-blue-500 text-white'
        }`}
        title={isPlaying ? 'Pause (Space)' : 'Play (Space)'}
      >
        {isPlaying ? '⏸ Pause' : '▶ Play'}
      </button>
      <button
        onClick={handleStepForward}
        className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-sm font-medium transition-colors"
        title="Step Forward (→)"
      >
        Step ▶
      </button>

      <div className="flex items-center gap-2 ml-2">
        <label className="text-xs text-slate-400">Speed:</label>
        <select
          ref={speedRef}
          value={playback.speed}
          onChange={handleSpeedChange}
          className="bg-slate-800 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
        >
          {SPEEDS.map((s) => (
            <option key={s} value={s}>
              {s}x
            </option>
          ))}
        </select>
      </div>

      <div className="flex-1 flex items-center gap-3 mx-4">
        <input
          type="range"
          min={0}
          max={playback.totalBars || 100}
          value={playback.currentBarIndex}
          onChange={handleProgressChange}
          className="flex-1 h-2 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-blue-500"
        />
        <div className="w-32 text-xs text-slate-400 text-right">
          {playback.currentBarIndex.toLocaleString()} / {playback.totalBars.toLocaleString()}
        </div>
      </div>

      <div className="text-xs text-slate-400 font-mono">
        {playback.currentTime
          ? new Date(playback.currentTime).toLocaleTimeString()
          : '--:--:--'}
      </div>
    </div>
  );
}
