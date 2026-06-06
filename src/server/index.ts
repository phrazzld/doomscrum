import fs from 'node:fs/promises';
import http, { type IncomingMessage, type ServerResponse } from 'node:http';
import path from 'node:path';
import { scanBacklog } from '../core/backlog';
import { appendDecision, createRunPacket } from '../core/events';
import { renderPrd } from '../core/pipeline';
import { getAppState } from '../core/state';
import { readRenders } from '../core/render-store';
import { distDir, rendersDir } from '../lib/paths';
import { FakeVideoProvider } from '../providers/fake';
import type { FeedDecision } from '../shared/types';

type Handler = (req: IncomingMessage, res: ServerResponse, url: URL) => Promise<void>;

function sendJson(res: ServerResponse, status: number, value: unknown): void {
  const body = JSON.stringify(value);
  res.writeHead(status, {
    'content-type': 'application/json',
    'content-length': Buffer.byteLength(body)
  });
  res.end(body);
}

async function readBody<T>(req: IncomingMessage): Promise<T> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  const raw = Buffer.concat(chunks).toString('utf8');
  return raw ? (JSON.parse(raw) as T) : ({} as T);
}

function contentType(filePath: string): string {
  if (filePath.endsWith('.html')) return 'text/html';
  if (filePath.endsWith('.js')) return 'text/javascript';
  if (filePath.endsWith('.css')) return 'text/css';
  if (filePath.endsWith('.mp4')) return 'video/mp4';
  if (filePath.endsWith('.png')) return 'image/png';
  return 'application/octet-stream';
}

async function sendFile(res: ServerResponse, filePath: string): Promise<void> {
  const data = await fs.readFile(filePath);
  res.writeHead(200, {
    'content-type': contentType(filePath),
    'content-length': data.length
  });
  res.end(data);
}

async function serveStatic(res: ServerResponse, root: string, requestPath: string): Promise<boolean> {
  const clean = decodeURIComponent(requestPath).replace(/^\/+/, '');
  const full = path.resolve(root, clean);
  if (!full.startsWith(path.resolve(root))) {
    sendJson(res, 403, { error: 'Forbidden' });
    return true;
  }
  try {
    await sendFile(res, full);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

const handlers: Record<string, Handler> = {
  'GET /api/state': async (_req, res) => {
    sendJson(res, 200, await getAppState());
  },
  'POST /api/render/fake': async (_req, res) => {
    const prds = await scanBacklog();
    const provider = new FakeVideoProvider();
    const renders = [];
    for (const prd of prds) {
      renders.push(await renderPrd(prd, provider));
    }
    sendJson(res, 200, { renders });
  },
  'POST /api/decision': async (req, res) => {
    const body = await readBody<Partial<FeedDecision>>(req);
    if (!body.prdId || !body.prdSha256 || !body.decision) {
      sendJson(res, 400, { error: 'prdId, prdSha256, and decision are required.' });
      return;
    }
    const event = await appendDecision({
      prdId: body.prdId,
      prdSha256: body.prdSha256,
      renderId: body.renderId,
      decision: body.decision,
      note: body.note,
      metadata: body.metadata
    });
    sendJson(res, 200, { event });
  },
  'POST /api/run-intent': async (req, res) => {
    const { prdId, renderId } = await readBody<{ prdId?: string; renderId?: string }>(req);
    if (!prdId) {
      sendJson(res, 400, { error: 'prdId is required.' });
      return;
    }
    const prd = (await scanBacklog()).find((item) => item.id === prdId);
    if (!prd) {
      sendJson(res, 404, { error: 'PRD not found.' });
      return;
    }
    const render = (await readRenders()).find((item) => item.id === renderId);
    const packet = await createRunPacket(prd, render);
    sendJson(res, 200, { packet });
  }
};

async function requestHandler(req: IncomingMessage, res: ServerResponse): Promise<void> {
  try {
    const url = new URL(req.url || '/', 'http://127.0.0.1');
    const key = `${req.method || 'GET'} ${url.pathname}`;
    const handler = handlers[key];
    if (handler) {
      await handler(req, res, url);
      return;
    }

    if (req.method === 'GET' && url.pathname.startsWith('/api/source/')) {
      const prdId = decodeURIComponent(url.pathname.replace('/api/source/', ''));
      const prd = (await scanBacklog()).find((item) => item.id === prdId);
      if (!prd) {
        sendJson(res, 404, { error: 'PRD not found.' });
        return;
      }
      sendJson(res, 200, { prd });
      return;
    }

    if (req.method === 'GET' && url.pathname.startsWith('/media/')) {
      const served = await serveStatic(res, rendersDir, url.pathname.replace('/media/', ''));
      if (served) return;
    }

    if (req.method === 'GET') {
      const served = await serveStatic(res, distDir, url.pathname === '/' ? 'index.html' : url.pathname);
      if (served) return;
      await sendFile(res, path.join(distDir, 'index.html'));
      return;
    }

    sendJson(res, 404, { error: 'Not found' });
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown server error.';
    sendJson(res, 500, { error: message });
  }
}

export const app = {
  listen(port: number, host: string, callback: () => void) {
    return http.createServer((req, res) => {
      void requestHandler(req, res);
    }).listen(port, host, callback);
  }
};
