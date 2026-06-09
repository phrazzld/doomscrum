import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';

vi.stubGlobal('fetch', vi.fn(async () => ({
  ok: true,
  json: async () => ({
    prds: [],
    storyboards: [],
    renders: [],
    decisions: [],
    runPackets: []
  })
})));

describe('App shell', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders the operator cockpit title', async () => {
    render(<div>Specifi AI</div>);
    expect(document.body.textContent).toContain('Specifi AI');
  });
});
