import { type RefObject, useRef, useEffect } from 'react';
import { EngineWebSocket } from '../../stores/websocket';
import { useAppStore } from '../../stores/useAppStore';

interface PlaybackPanelProps {
  wsRef: RefObject<EngineWebSocket | null>;
}

const SPEED_OPTIONS: Array<number | 'max'> = [0.5, 1, 3, 10, 'max'];

function getInterval(speed: number | 'max'): number {
  switch (speed) {
    case 'max': return 6;
    case 0.5: return 2000;
    case 1: return 333;
    case 3: return 100;
    case 10: return 50;
    default: return Math.max(200, 1000 / speed);
  }
}

export function PlaybackPanel({ wsRef }: PlaybackPanelProps) {
  const { playback, setPlayback, currentStrategy } = useAppStore();
  const { status, speed } = playback;

  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (status === 'complete' && intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }, [status]);

  useEffect(() => {
    if (
      playback.currentBarIndex >= playback.totalBars &&
      playback.totalBars > 0 &&
      status !== 'complete'
    ) {
      setPlayback({ status: 'complete' });
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    }
  }, [playback.currentBarIndex, playback.totalBars, status]);

  const handlePlay = () => {
    if (intervalRef.current) return;
    const isAtEnd = playback.currentBarIndex >= playback.totalBars && playback.totalBars > 0;
    if (isAtEnd) {
      setPlayback({ status: 'complete' });
      return;
    }
    setPlayback({ status: 'playing' });
    const playStep = () => wsRef.current?.sendControl('play');
    playStep();
    const ms = getInterval(speed);
    intervalRef.current = setInterval(playStep, ms);
  };

  const handlePause = () => {
    wsRef.current?.sendControl('pause');
    setPlayback({ status: 'paused' });
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  };

  const handleStepForward = () => {
    const isAtEnd = playback.currentBarIndex >= playback.totalBars && playback.totalBars > 0;
    if (isAtEnd) {
      setPlayback({ status: 'complete' });
      return;
    }
    wsRef.current?.sendControl('step_forward');
  };

  const handleStepBackward = () => {
    wsRef.current?.sendControl('step_backward');
  };

  const handleSpeedChange = (newSpeed: number | 'max') => {
    setPlayback({ speed: newSpeed });
    wsRef.current?.setSpeed(newSpeed);
  };

  const formatTime = (timestamp: number) => {
    if (!timestamp) return '--:--:--';
    return new Date(timestamp * 1000).toLocaleTimeString();
  };

  return (
    <div className="panel flex-shrink-0">
      <div className="panel-header">
        <span className="panel-title">Playback Control</span>
      </div>

      <div className="panel-body space-y-4">
        <div className="flex items-center justify-center gap-2">
          <button
            onClick={handleStepBackward}
            className="btn-icon"
            disabled={status === 'complete'}
            title="Step Backward"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12.066 11.2a1 1 0 000 1.6l5.334 4A1 1 0 0019 16V8a1 1 0 00-1.6-.8l-5.333 4zM4.066 11.2a1 1 0 000 1.6l5.334 4A1 1 0 0011 16V8a1 1 0 00-1.6-.8l-5.334 4z" />
            </svg>
          </button>

          {status === 'playing' ? (
            <button
              onClick={handlePause}
              className="btn-icon p-2"
              title="Pause"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path d="M6 4h4v16H6V4zm8 0h4v16h-4V4z" />
              </svg>
            </button>
          ) : (
            <button
              onClick={handlePlay}
              className="btn-icon p-2"
              disabled={status === 'complete'}
              title={status === 'complete' ? 'Backtest Complete' : 'Play'}
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z" />
              </svg>
            </button>
          )}

          <button
            onClick={handleStepForward}
            className="btn-icon"
            disabled={status === 'complete'}
            title="Step Forward"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11.933 12.8a1 1 0 000-1.6L6.6 7.2A1 1 0 005 8v8a1 1 0 001.6.8l5.333-4zM19.933 12.8a1 1 0 000-1.6l-5.333-4A1 1 0 0013 8v8a1 1 0 001.6.8l5.333-4z" />
            </svg>
          </button>
        </div>

        <div className="space-y-2">
          <label className="label">Speed</label>
          <div className="flex gap-1">
            {SPEED_OPTIONS.map((s) => (
              <button
                key={s}
                onClick={() => handleSpeedChange(s)}
                className={`flex-1 py-1 text-2xs font-mono rounded transition-colors ${
                  speed === s
                    ? 'bg-accent-cyan text-surface-base'
                    : 'bg-surface-raised text-text-secondary hover:bg-surface-elevated'
                }`}
              >
                {s === 'max' ? 'max' : `${s}x`}
              </button>
            ))}
          </div>
        </div>

        {currentStrategy && (
          <div className="bg-surface-raised rounded p-3 space-y-2">
            <div className="text-xs font-medium text-text-primary">{currentStrategy.name}</div>
            {Object.entries(currentStrategy.default_params).length > 0 && (
              <div className="space-y-1">
                {Object.entries(currentStrategy.default_params).map(([key, value]) => (
                  <div key={key} className="flex justify-between text-xs">
                    <span className="text-text-secondary">{key}</span>
                    <span className="font-mono text-text-primary">{String(value)}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        <div className="grid grid-cols-2 gap-2 text-center">
          <div className="bg-surface-raised rounded p-2">
            <div className="text-2xs text-text-muted mb-0.5">Position</div>
            <div className="text-sm font-mono tabular-nums text-text-primary">
              {playback.currentBarIndex + 1} / {playback.totalBars || '--'}
            </div>
          </div>
          <div className="bg-surface-raised rounded p-2">
            <div className="text-2xs text-text-muted mb-0.5">Time</div>
            <div className="text-sm font-mono tabular-nums text-text-primary">
              {formatTime(playback.currentTime)}
            </div>
          </div>
        </div>

        {status === 'complete' && (
          <div className="bg-accent-green/10 border border-accent-green/30 rounded p-2 text-center">
            <span className="text-xs font-medium text-accent-green">
              BACKTEST COMPLETE
            </span>
          </div>
        )}
      </div>
    </div>
  );
}