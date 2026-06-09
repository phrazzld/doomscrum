import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { ArrowDown, ArrowLeft, ArrowRight, Bot, Eye, FileText, Film, RefreshCw, Shield, Sparkles } from 'lucide-react';
import type { AppState, FeedDecision, PrdSource, Storyboard, VideoRender } from './shared/types';
import './styles.css';

const emptyState: AppState = {
  prds: [],
  storyboards: [],
  renders: [],
  decisions: [],
  runPackets: [],
  providerConfigured: false,
  config: {
    defaults: {
      repoPath: '.',
      allowedCommands: [],
      agentMode: 'packet-only',
      renderProvider: 'fake',
      maxRenderSpendUsd: 0
    },
    items: {}
  }
};

async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init);
  if (!response.ok) {
    const body = (await response.json().catch(() => ({ error: response.statusText }))) as { error?: string };
    throw new Error(body.error || response.statusText);
  }
  return response.json() as Promise<T>;
}

function short(hash: string): string {
  return hash.slice(0, 10);
}

function latestRender(prd: PrdSource, renders: VideoRender[]): VideoRender | undefined {
  const ready = renders.filter((render) => render.prdId === prd.id && render.status === 'ready');
  return ready.find((render) => render.provider !== 'fake-local') || ready[0];
}

function storyboardFor(prd: PrdSource, storyboards: Storyboard[]): Storyboard | undefined {
  return storyboards.find((storyboard) => storyboard.prdId === prd.id);
}

function App(): React.ReactElement {
  const [state, setState] = useState<AppState>(emptyState);
  const [selectedId, setSelectedId] = useState<string>('');
  const [inspectOpen, setInspectOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('Booting the slop cannon...');

  async function load(): Promise<void> {
    const next = await api<AppState>('/api/state');
    setState(next);
    setSelectedId((current) => current || next.prds[0]?.id || '');
    setMessage(`${next.prds.length} PRDs indexed, ${next.renders.length} renders found`);
  }

  useEffect(() => {
    void load();
  }, []);

  const selected = useMemo(() => state.prds.find((prd) => prd.id === selectedId) || state.prds[0], [selectedId, state.prds]);
  const render = selected ? latestRender(selected, state.renders) : undefined;
  const storyboard = selected ? storyboardFor(selected, state.storyboards) : undefined;

  async function renderFake(): Promise<void> {
    setBusy(true);
    setMessage('Generating fake native-audio MP4s...');
    try {
      await api('/api/render/fake', { method: 'POST' });
      await load();
      setMessage('Fake provider renders are ready');
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Render failed');
    } finally {
      setBusy(false);
    }
  }

  async function renderReal(): Promise<void> {
    if (!selected) {
      return;
    }
    setBusy(true);
    setMessage(`Generating AI video for ${selected.title}...`);
    try {
      await api('/api/render/real', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prdId: selected.id })
      });
      await load();
      setMessage(`AI video ready for ${selected.title}`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'AI render failed');
    } finally {
      setBusy(false);
    }
  }

  async function decision(decisionType: FeedDecision['decision'], note?: string): Promise<void> {
    if (!selected) {
      return;
    }
    setBusy(true);
    try {
      if (decisionType === 'run_intent') {
        await api('/api/run-intent', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ prdId: selected.id, renderId: render?.id })
        });
        await load();
        setMessage(`Run packet created for ${selected.title}`);
      } else {
        await api('/api/decision', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            prdId: selected.id,
            prdSha256: selected.sha256,
            renderId: render?.id,
            decision: decisionType,
            note
          })
        });
        await load();
        setMessage(`${decisionType.replace('_', ' ')} recorded for ${selected.title}`);
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Decision failed');
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="mark">BR</div>
          <div>
            <h1>PRD Brainrot Swipe</h1>
            <p>local backlog.d triage</p>
          </div>
        </div>
        <button className="primary-action" onClick={renderReal} disabled={busy || !selected || !state.providerConfigured}>
          <Sparkles size={16} />
          Generate AI video
        </button>
        <button className="secondary-action" onClick={renderFake} disabled={busy}>
          <RefreshCw size={16} />
          QA fixture MP4s
        </button>
        <div className="list" data-testid="prd-list">
          {state.prds.map((prd) => (
            <button
              key={prd.id}
              className={`prd-row ${prd.id === selected?.id ? 'selected' : ''}`}
              onClick={() => {
                setSelectedId(prd.id);
                setInspectOpen(false);
              }}
            >
              <span className={`status-dot ${prd.status}`} />
              <span>
                <strong>{prd.title}</strong>
                <small>{short(prd.sha256)} · {prd.status}</small>
              </span>
            </button>
          ))}
        </div>
      </aside>

      <section className="stage">
        <div className="topline">
          <span>{message}</span>
          <span className="remote-warning"><Shield size={15} /> {state.providerConfigured ? 'FAL provider configured' : 'FAL_API_KEY required for AI video'}</span>
        </div>

        <div className="phone-frame" data-testid="video-card">
          {render?.assetUrl ? (
            <video controls loop src={render.assetUrl} aria-label={`Generated video for ${selected?.title}`} />
          ) : (
            <div className="empty-video">
              <Film size={56} />
              <strong>No MP4 yet</strong>
              <span>Generate an AI video or QA fixture to load the feed.</span>
            </div>
          )}
          <div className="caption-stack">
            <span className="caption-hot">goofy spec payload</span>
            <strong>{selected?.title || 'No PRD selected'}</strong>
            <span>{render ? `${render.provider}/${render.model}` : 'provider pending'}</span>
          </div>
        </div>

        <div className="gesture-bar" aria-label="Swipe decisions">
          <button onClick={() => setInspectOpen((open) => !open)} disabled={!selected}>
            <Eye size={18} /> Inspect
          </button>
          <button onClick={() => void decision('needs_spec', 'Clarification required before agent work.')} disabled={!selected || busy}>
            <ArrowLeft size={18} /> Needs spec
          </button>
          <button onClick={() => void decision('skip')} disabled={!selected || busy}>
            <ArrowDown size={18} /> Skip
          </button>
          <button className="run" onClick={() => void decision('run_intent')} disabled={!selected || busy}>
            <ArrowRight size={18} /> Launch Codex
          </button>
        </div>

        <div className="render-strip">
          {state.renders.slice(0, 6).map((item) => (
            <button key={item.id} onClick={() => setSelectedId(item.prdId)} className={item.prdId === selected?.id ? 'active' : ''}>
              <Sparkles size={14} />
              {short(item.prdSha256)} · {item.audioMode}
            </button>
          ))}
        </div>
      </section>

      <aside className="inspector">
        <div className="panel">
          <h2><FileText size={18} /> Source spec</h2>
          <dl>
            <dt>Path</dt>
            <dd>{selected?.path || '-'}</dd>
            <dt>Hash</dt>
            <dd>{selected?.sha256 || '-'}</dd>
            <dt>Status</dt>
            <dd>{selected?.status || '-'}</dd>
          </dl>
        </div>

        <div className="panel">
          <h2><Film size={18} /> Render provenance</h2>
          <dl>
            <dt>Provider</dt>
            <dd>{render ? `${render.provider}/${render.model}` : '-'}</dd>
            <dt>Audio mode</dt>
            <dd>{render?.audioMode || '-'}</dd>
            <dt>Cost</dt>
            <dd>{render?.actualCostUsd ?? render?.costEstimateUsd ?? '-'}</dd>
            <dt>Latency</dt>
            <dd>{render?.latencyMs ? `${render.latencyMs}ms` : '-'}</dd>
            <dt>Storyboard hash</dt>
            <dd>{render?.storyboardHash || '-'}</dd>
          </dl>
        </div>

        <div className="panel beats">
          <h2><Bot size={18} /> Storyboard beats</h2>
          {storyboard?.beats.map((beat) => (
            <article key={beat.label}>
              <strong>{beat.label}</strong>
              <p>{beat.caption}</p>
            </article>
          )) || <p>No storyboard yet.</p>}
        </div>

        {inspectOpen && selected ? (
          <div className="panel source-panel" data-testid="source-panel">
            <h2>Exact PRD</h2>
            <pre>{selected.raw}</pre>
          </div>
        ) : null}
      </aside>
    </main>
  );
}

createRoot(document.getElementById('root') as HTMLElement).render(<App />);
