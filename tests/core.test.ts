import fs from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { scanBacklog } from '../src/core/backlog';
import { distillPrd, compileStoryboard } from '../src/core/distill';
import { appendDecision, createRunPacket } from '../src/core/events';
import { renderPrd } from '../src/core/pipeline';
import { readRenders } from '../src/core/render-store';
import { eventsPath, runPacketsDir } from '../src/lib/paths';
import { FakeVideoProvider } from '../src/providers/fake';

describe('PRD brainrot pipeline', () => {
  it('indexes local backlog PRDs with hashes', async () => {
    const prds = await scanBacklog();
    expect(prds.length).toBeGreaterThanOrEqual(5);
    expect(prds[0]?.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(prds[0]?.path).toContain('backlog.d/');
  });

  it('distills PRDs into a storyboard without unsupported shipping claims', async () => {
    const [prd] = await scanBacklog();
    expect(prd).toBeDefined();
    const brief = distillPrd(prd);
    const storyboard = compileStoryboard(prd, brief);
    expect(storyboard.providerPrompt).toContain(prd.title);
    expect(storyboard.providerPrompt).toContain('Do not invent shipped features');
    expect(storyboard.beats.length).toBeGreaterThanOrEqual(5);
  });

  it('fake provider writes playable MP4 provenance with native audio mode', async () => {
    const [prd] = await scanBacklog();
    const render = await renderPrd(prd, new FakeVideoProvider());
    expect(render.audioMode).toBe('native');
    expect(render.nativeAudioRequested).toBe(true);
    expect(render.provider).toBe('fake-local');
    expect(render.storyboardHash).toMatch(/^[a-f0-9]{64}$/);
    expect(render.assetPath).toBeDefined();
    const stat = await fs.stat(render.assetPath as string);
    expect(stat.size).toBeGreaterThan(1000);
    const renders = await readRenders();
    expect(renders.some((item) => item.id === render.id)).toBe(true);
  });

  it('records needs-spec and skip decisions as durable events', async () => {
    const [, prd] = await scanBacklog();
    const needsSpec = await appendDecision({
      prdId: prd.id,
      prdSha256: prd.sha256,
      decision: 'needs_spec',
      note: 'Test clarification event.'
    });
    const skip = await appendDecision({
      prdId: prd.id,
      prdSha256: prd.sha256,
      decision: 'skip'
    });
    const raw = await fs.readFile(eventsPath, 'utf8');
    expect(raw).toContain(needsSpec.id);
    expect(raw).toContain(skip.id);
  });

  it('right-swipe creates a bounded run packet instead of launching an agent', async () => {
    const [prd] = await scanBacklog();
    const render = await renderPrd(prd, new FakeVideoProvider());
    const packet = await createRunPacket(prd, render);
    expect(packet.status).toBe('created');
    expect(packet.timeoutSec).toBe(180);
    expect(packet.allowedCommands).toEqual(['npm test', 'npm run typecheck', 'npm run lint']);
    const packetPath = path.join(runPacketsDir, `${packet.id}.json`);
    const raw = await fs.readFile(packetPath, 'utf8');
    expect(raw).toContain(prd.sha256);
  });
});
