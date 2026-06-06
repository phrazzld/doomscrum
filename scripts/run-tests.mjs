import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();

function run(command, args) {
  execFileSync(command, args, { cwd: root, stdio: 'pipe' });
}

async function waitForServer(url, child) {
  const started = Date.now();
  while (Date.now() - started < 10_000) {
    if (child.exitCode !== null) {
      throw new Error(`server exited early with ${child.exitCode}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
  }
  throw new Error(`server did not become ready at ${url}`);
}

run('node', ['scripts/build-server.mjs']);
run('node', ['build-server/generate.js']);

const backlogFiles = (await fs.readdir(path.join(root, 'backlog.d'))).filter((file) => file.endsWith('.md'));
assert.equal(backlogFiles.length, 5, 'expected five sample PRDs');

const storyboardFiles = await fs.readdir(path.join(root, '.brainrot', 'storyboards'));
assert.equal(storyboardFiles.filter((file) => file.endsWith('.json')).length, 5, 'expected five storyboards');

const renderRoot = path.join(root, '.brainrot', 'renders');
const renderDirs = await fs.readdir(renderRoot);
const renderJson = [];
for (const dir of renderDirs) {
  for (const file of await fs.readdir(path.join(renderRoot, dir))) {
    if (file.endsWith('.json')) {
      renderJson.push(JSON.parse(await fs.readFile(path.join(renderRoot, dir, file), 'utf8')));
    }
  }
}
assert.equal(renderJson.length, 5, 'expected five render provenance files');
for (const render of renderJson) {
  assert.equal(render.audioMode, 'native');
  assert.equal(render.nativeAudioRequested, true);
  assert.equal(render.provider, 'fake-local');
  assert.match(render.storyboardHash, /^[a-f0-9]{64}$/);
  assert.ok(render.assetPath, 'render must include assetPath');
  const stat = await fs.stat(render.assetPath);
  assert.ok(stat.size > 1000, 'render MP4 should be non-empty');
}

const server = spawn('node', ['build-server/start.js'], { cwd: root, stdio: 'pipe' });
try {
  await waitForServer('http://127.0.0.1:4173/api/state', server);
  const state = await fetch('http://127.0.0.1:4173/api/state').then((response) => response.json());
  assert.equal(state.prds.length, 5);
  assert.ok(state.renders.length >= 5);
  const first = state.prds[0];
  const firstRender = state.renders.find((render) => render.prdId === first.id);
  assert.ok(firstRender, 'first PRD should have a render');

  const needsSpec = await fetch('http://127.0.0.1:4173/api/decision', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      prdId: first.id,
      prdSha256: first.sha256,
      renderId: firstRender.id,
      decision: 'needs_spec',
      note: 'acceptance test needs-spec event'
    })
  }).then((response) => response.json());
  assert.equal(needsSpec.event.decision, 'needs_spec');

  const runIntent = await fetch('http://127.0.0.1:4173/api/run-intent', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ prdId: first.id, renderId: firstRender.id })
  }).then((response) => response.json());
  assert.equal(runIntent.packet.status, 'created');
  assert.equal(runIntent.packet.timeoutSec, 180);
  assert.deepEqual(runIntent.packet.allowedCommands, ['npm test', 'npm run typecheck', 'npm run lint']);
  const packetPath = path.join(root, '.brainrot', 'run-packets', `${runIntent.packet.id}.json`);
  await fs.access(packetPath);
} finally {
  if (server.exitCode === null) {
    server.kill();
    await new Promise((resolve) => {
      server.once('exit', resolve);
      setTimeout(resolve, 1000);
    });
  }
}

console.log('acceptance tests passed');
