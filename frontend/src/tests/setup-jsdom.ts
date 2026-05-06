import { JSDOM } from 'jsdom';

// Setup jsdom for test environments that don't provide it (e.g., bun test)
// This MUST run before any @testing-library imports to ensure document is available
if (typeof document === 'undefined') {
  const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>', {
    url: 'http://localhost',
  });
  globalThis.window = dom.window as unknown as Window & typeof globalThis;
  globalThis.document = dom.window.document;
  globalThis.navigator = dom.window.navigator;
}
