import "@testing-library/jest-dom";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

// Pin language at module load time (before any test code imports the i18n
// module). Tests assert against zh-CN strings — the same default used in
// production for this locale. Setting these at the top level (not inside
// beforeAll) ensures the i18n init sees the right values when it first
// evaluates navigator.language / localStorage.
if (typeof navigator !== "undefined") {
  Object.defineProperty(navigator, "language", {
    configurable: true,
    get: () => "zh-CN",
  });
}
if (typeof window !== "undefined" && window.localStorage) {
  try {
    window.localStorage.setItem("last-token.lang", "zh-CN");
  } catch {
    // Ignore storage errors; the navigator.language override above is enough.
  }
}

// Clean up after each test
afterEach(() => {
  cleanup();
});

// Mock ResizeObserver for Recharts
class MockResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

(globalThis as any).ResizeObserver = MockResizeObserver;

// Mock matchMedia for shadcn components
if (typeof window !== "undefined" && !window.matchMedia) {
  window.matchMedia = (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  });
}

// Mock getBoundingClientRect for Recharts ResponsiveContainer
if (typeof Element !== "undefined") {
  Element.prototype.getBoundingClientRect = function () {
    return {
      width: 500,
      height: 200,
      top: 0,
      left: 0,
      right: 500,
      bottom: 200,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    } as DOMRect;
  };
}
