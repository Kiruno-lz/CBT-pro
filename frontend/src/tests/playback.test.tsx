import { describe, it, expect, vi } from 'vitest';
import { create } from 'zustand';
import type {
  StandardBar,
  EngineSnapshot,
  Signal,
  PlaybackState,
  BacktestResult,
} from '../types';

// Minimal reproduction of the playback store for isolated testing
interface TestPlaybackState {
  playback: PlaybackState;
  setPlayback: (update: Partial<PlaybackState>) => void;
  wsConnected: boolean;
  setWsConnected: (connected: boolean) => void;
}

const useTestPlaybackStore = create<TestPlaybackState>((set) => ({
  playback: {
    status: 'idle',
    currentBarIndex: 0,
    totalBars: 0,
    speed: 1,
    currentTime: 0,
  },
  setPlayback: (update) =>
    set((state) => ({
      playback: { ...state.playback, ...update },
    })),
  wsConnected: false,
  setWsConnected: (connected) => set({ wsConnected: connected }),
}));

describe('Playback Store State Transitions', () => {
  it('initializes with idle status', () => {
    const state = useTestPlaybackStore.getState();
    expect(state.playback.status).toBe('idle');
    expect(state.playback.currentBarIndex).toBe(0);
    expect(state.playback.totalBars).toBe(0);
    expect(state.playback.speed).toBe(1);
  });

  it('transitions from idle to playing', () => {
    useTestPlaybackStore.getState().setPlayback({ status: 'playing' });
    const state = useTestPlaybackStore.getState();
    expect(state.playback.status).toBe('playing');
  });

  it('transitions from playing to paused', () => {
    useTestPlaybackStore.getState().setPlayback({ status: 'playing' });
    useTestPlaybackStore.getState().setPlayback({ status: 'paused' });
    const state = useTestPlaybackStore.getState();
    expect(state.playback.status).toBe('paused');
  });

  it('transitions from paused to stepping', () => {
    useTestPlaybackStore.getState().setPlayback({ status: 'paused' });
    useTestPlaybackStore.getState().setPlayback({ status: 'stepping' });
    const state = useTestPlaybackStore.getState();
    expect(state.playback.status).toBe('stepping');
  });

  it('transitions to complete', () => {
    useTestPlaybackStore.getState().setPlayback({
      status: 'complete',
      currentBarIndex: 99,
      totalBars: 100,
    });
    const state = useTestPlaybackStore.getState();
    expect(state.playback.status).toBe('complete');
    expect(state.playback.currentBarIndex).toBe(99);
    expect(state.playback.totalBars).toBe(100);
  });

  it('updates bar index during playback', () => {
    useTestPlaybackStore.getState().setPlayback({
      status: 'playing',
      currentBarIndex: 5,
      totalBars: 100,
      currentTime: 1704067200000,
    });
    const state = useTestPlaybackStore.getState();
    expect(state.playback.currentBarIndex).toBe(5);
    expect(state.playback.totalBars).toBe(100);
    expect(state.playback.currentTime).toBe(1704067200000);
  });

  it('updates playback speed', () => {
    useTestPlaybackStore.getState().setPlayback({ speed: 5 });
    const state = useTestPlaybackStore.getState();
    expect(state.playback.speed).toBe(5);
  });

  it('tracks ws connection state', () => {
    useTestPlaybackStore.getState().setWsConnected(true);
    const state = useTestPlaybackStore.getState();
    expect(state.wsConnected).toBe(true);
  });

  it('preserves other fields when updating one', () => {
    useTestPlaybackStore.setState({
      playback: {
        status: 'playing',
        currentBarIndex: 42,
        totalBars: 1000,
        speed: 2,
        currentTime: 1704067200000,
      },
    });

    useTestPlaybackStore.getState().setPlayback({ speed: 10 });

    const state = useTestPlaybackStore.getState();
    expect(state.playback.status).toBe('playing');
    expect(state.playback.currentBarIndex).toBe(42);
    expect(state.playback.totalBars).toBe(1000);
    expect(state.playback.speed).toBe(10);
    expect(state.playback.currentTime).toBe(1704067200000);
  });
});
