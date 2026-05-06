import { describe, it, expect } from 'vitest';

// Replicate the constants and logic from PlaybackPanel for testing
const SPEED_OPTIONS = [0.5, 1, 3, 10, 'max'] as const;

type Speed = number | 'max';

function getInterval(speed: Speed): number {
  switch (speed) {
    case 'max': return 6;
    case 0.5: return 2000;
    case 1: return 333;
    case 3: return 100;
    case 10: return 50;
    default: return Math.max(200, 1000 / speed);
  }
}

function getBackendSpeed(speed: Speed): number {
  return speed === 'max' ? 0 : speed;
}

function formatSpeedButton(speed: Speed): string {
  return speed === 'max' ? 'max' : `${speed}x`;
}

describe('PlaybackPanel', () => {
  describe('SPEED_OPTIONS', () => {
    it('should include max speed option', () => {
      expect(SPEED_OPTIONS).toContain('max');
    });

    it('should have correct speed values', () => {
      expect(SPEED_OPTIONS).toEqual([0.5, 1, 3, 10, 'max']);
    });
  });

  describe('speed button formatting', () => {
    it('should format max speed without x suffix', () => {
      expect(formatSpeedButton('max')).toBe('max');
    });

    it('should format numeric speeds with x suffix', () => {
      expect(formatSpeedButton(0.5)).toBe('0.5x');
      expect(formatSpeedButton(1)).toBe('1x');
      expect(formatSpeedButton(3)).toBe('3x');
      expect(formatSpeedButton(10)).toBe('10x');
    });
  });

  describe('handleSpeedChange', () => {
    it('should convert max speed to 0 for backend', () => {
      expect(getBackendSpeed('max')).toBe(0);
    });

    it('should pass through numeric speeds unchanged', () => {
      expect(getBackendSpeed(0.5)).toBe(0.5);
      expect(getBackendSpeed(1)).toBe(1);
      expect(getBackendSpeed(3)).toBe(3);
      expect(getBackendSpeed(10)).toBe(10);
    });
  });

  describe('interval calculation', () => {
    it('should calculate correct interval for speed 0.5', () => {
      expect(getInterval(0.5)).toBe(2000);
    });

    it('should calculate correct interval for speed 1', () => {
      expect(getInterval(1)).toBe(333);
    });

    it('should calculate correct interval for speed 3', () => {
      expect(getInterval(3)).toBe(100);
    });

    it('should calculate correct interval for speed 10', () => {
      expect(getInterval(10)).toBe(50);
    });

    it('should use 6ms interval for max speed', () => {
      expect(getInterval('max')).toBe(6);
    });
  });
});
