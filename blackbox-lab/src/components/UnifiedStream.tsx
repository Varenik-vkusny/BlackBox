import { useState, useRef, useEffect, useMemo, useCallback } from 'react';
import { ArrowLeft } from 'lucide-react';
import { FixedSizeList as List } from 'react-window';
import type { ListChildComponentProps } from 'react-window';
import AutoSizer from 'react-virtualized-auto-sizer';

import type {
  DockerResponse,
  HttpErrorsResponse,
  CompressedResponse,
  DiffResponse,
  StructuredResponse,
  StreamEntry,
} from '../types';
import { NetworkInspector } from './NetworkInspector';
import { GitLens } from './GitLens';

interface Props {
  logs: string[];
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  compressed: CompressedResponse | null;
  diff: DiffResponse | null;
  structured: StructuredResponse | null;
  selectedSource: string | null;
  timeFilter: number | null;
  onCorrelate: (entry: StreamEntry) => void;
  onInspectDiff: () => void;
  onSpanSearch: (spanId: string) => void;
  onBack?: () => void;
}

type TabKind = 'all' | 'terminal' | 'docker' | 'network' | 'compressed' | 'diff';

// ── Display item types ───────────────────────────────────────────────────────

type DisplayItem =
  | { kind: 'line'; entry: StreamEntry }
  | { kind: 'stack'; id: string; header: string; lang: string; frames: string[]; timestamp_ms: number }
  | { kind: 'dedup'; id: string; count: number; representative: StreamEntry };

// ── Helpers ──────────────────────────────────────────────────────────────────

function levelOfLine(text: string): string | null {
  const t = text.toLowerCase();
  if (/\b(fatal|panic)\b/.test(t)) return 'fatal';
  if (/\berror\b/.test(t)) return 'error';
  if (/\bwarn(ing)?\b/.test(t)) return 'warn';
  return null;
}

function timeLabel(ms: number): string {
  return new Date(ms).toTimeString().slice(0, 8);
}

function timeAgo(ms: number): string {
  const s = Math.floor((Date.now() - ms) / 1000);
  if (s < 60) return `${s}s ago`;
  return `${Math.floor(s / 60)}m ago`;
}

const FRAME_RE = /^\s+(at\s+.+:\d+:\d+|File\s+".+",\s+line\s+\d+|at\s+[\w$.]+\([\w$.]+\.java:\d+\)|\d+:\s+\w+(::\w+)+)/;

function isFrameLine(text: string): boolean {
  return FRAME_RE.test(text);
}

function detectLang(frames: string[]): string {
  for (const f of frames) {
    if (/File\s+"/.test(f)) return 'python';
    if (/\.java:\d+\)/.test(f)) return 'java';
    if (/:\d+:\d+\)/.test(f)) return 'nodejs';
    if (/\d+:\s+\w+::\w+/.test(f)) return 'rust';
  }
  return 'unknown';
}

// Pre-process entries into flat display items, grouping consecutive frame lines
function buildDisplayItems(entries: StreamEntry[]): DisplayItem[] {
  const items: DisplayItem[] = [];
  let i = 0;
  while (i < entries.length) {
    const e = entries[i];
    if (e.source_type === 'terminal' && isFrameLine(e.text)) {
      const header = i > 0 ? entries[i - 1].text : '';
      const frames: string[] = [];
      const startTs = e.timestamp_ms;
      while (i < entries.length && entries[i].source_type === 'terminal' && isFrameLine(entries[i].text)) {
        frames.push(entries[i].text);
        i++;
      }
      if (frames.length >= 2) {
        items.push({ kind: 'stack', id: `stack-${startTs}`, header, lang: detectLang(frames), frames, timestamp_ms: startTs });
        continue;
      }
      // Fewer than 2 frames: render as normal lines
      for (const f of frames) {
        items.push({ kind: 'line', entry: { ...e, text: f } });
      }
    } else {
      items.push({ kind: 'line', entry: e });
      i++;
    }
  }
  return items;
}

// Collapse consecutive identical-prefix lines into [xN] badges (threshold: 3+)
function deduplicateItems(items: DisplayItem[]): DisplayItem[] {
  const result: DisplayItem[] = [];
  let i = 0;
  while (i < items.length) {
    const item = items[i];
    if (item.kind !== 'line') { result.push(item); i++; continue; }

    const prefix = item.entry.text.slice(0, 60);
    let j = i + 1;
    while (
      j < items.length &&
      items[j].kind === 'line' &&
      (items[j] as { kind: 'line'; entry: StreamEntry }).entry.text.slice(0, 60) === prefix
    ) j++;

    const count = j - i;
    if (count >= 3) {
      result.push({ kind: 'dedup', id: `dedup-${item.entry.id}`, count, representative: item.entry });
    } else {
      for (let k = i; k < j; k++) result.push(items[k]);
    }
    i = j;
  }
  return result;
}

// ── Component ────────────────────────────────────────────────────────────────

export function UnifiedStream({
  logs, docker, httpErrors, compressed, diff, structured,
  selectedSource, timeFilter,
  onCorrelate, onInspectDiff, onSpanSearch,
  onBack,
}: Props) {
  const [tab, setTab] = useState<TabKind>('all');
  const [filter, setFilter] = useState('');
  const [smartCollapse, setSmartCollapse] = useState(false);
  const [showDiff, setShowDiff] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const listRef = useRef<List>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevLogsLen = useRef(0);

  // Span search side-effect: use useEffect, NOT useMemo
  useEffect(() => {
    if (filter.startsWith('span_id:')) {
      const sid = filter.slice(8).trim();
      if (sid) onSpanSearch(sid);
    }
  }, [filter, onSpanSearch]);

  // Auto-scroll when new lines arrive
  useEffect(() => {
    if (!autoScroll) return;
    if (logs.length === prevLogsLen.current) return;
    prevLogsLen.current = logs.length;
    // plain div scroll
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
    // virtualized list scroll
    if (listRef.current) {
      listRef.current.scrollToItem(logs.length - 1, 'end');
    }
  }, [logs.length, autoScroll]);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
    if (!nearBottom) setAutoScroll(false);
    else setAutoScroll(true);
  }, []);

  // Build merged stream entries
  const [now] = useState(() => Date.now());
  const allEntries: StreamEntry[] = useMemo(() => {
    const entries: StreamEntry[] = [];

    const termLines = logs.slice().reverse();
    const step = termLines.length > 1 ? 30000 / termLines.length : 0;
    termLines.forEach((text, i) => {
      entries.push({
        id: `t-${i}`,
        source_type: 'terminal',
        source_label: 'bash',
        text,
        timestamp_ms: now - i * step,
        level: levelOfLine(text),
      });
    });

    docker?.events?.forEach((ev, i) => {
      const short = ev.source.container_id.slice(0, 12);
      entries.push({
        id: `d-${i}`,
        source_type: 'docker',
        source_label: `docker:${short}`,
        text: ev.text,
        timestamp_ms: ev.timestamp_ms,
        level: ev.level,
        raw: ev,
      });
    });

    httpErrors?.events?.forEach((ev, i) => {
      entries.push({
        id: `h-${i}`,
        source_type: 'http',
        source_label: `http:${ev.status}`,
        text: `${ev.method} ${ev.url} → ${ev.status} (${ev.latency_ms}ms)`,
        timestamp_ms: ev.timestamp_ms,
        level: ev.status >= 500 ? 'error' : 'warn',
        raw: ev,
      });
    });

    return entries.sort((a, b) => b.timestamp_ms - a.timestamp_ms);
  }, [logs, docker, httpErrors]);

  // Filter entries
  const filteredEntries = useMemo(() => {
    let items = allEntries;

    if (tab === 'terminal') items = items.filter(e => e.source_type === 'terminal');
    else if (tab === 'docker') items = items.filter(e => e.source_type === 'docker');
    else if (tab === 'network') return [];

    if (selectedSource) {
      if (selectedSource === 'terminal') items = items.filter(e => e.source_type === 'terminal');
      else if (selectedSource === 'http') items = items.filter(e => e.source_type === 'http');
      else if (selectedSource.startsWith('docker:')) {
        const selectedCid = selectedSource.slice(7);
        items = items.filter(e => {
          if (!e.source_label.startsWith('docker:')) return false;
          const itemCid = e.source_label.slice(7);
          return selectedCid.startsWith(itemCid) || itemCid.startsWith(selectedCid);
        });
      }
    }

    if (filter.startsWith('span_id:')) {
      if (structured?.events?.length) {
        return structured.events.map((ev, i) => ({
          id: `s-${i}`,
          source_type: 'terminal' as const,
          source_label: `${ev.format}`,
          text: `[${ev.level ?? 'info'}] ${ev.message}${ev.span_id ? ` span=${ev.span_id}` : ''}`,
          timestamp_ms: ev.timestamp_ms,
          level: ev.level,
        }));
      }
      return [];
    } else if (filter) {
      const q = filter.toLowerCase();
      items = items.filter(e => e.text.toLowerCase().includes(q));
    }

    return items;
  }, [allEntries, tab, selectedSource, filter, structured]);

  // Build display items with stack block grouping + dedup collapse
  const displayItems = useMemo((): DisplayItem[] => {
    if (smartCollapse) return [];
    return deduplicateItems(buildDisplayItems(filteredEntries));
  }, [filteredEntries, smartCollapse]);

  const handleInspectDiff = useCallback(() => {
    onInspectDiff();
    setShowDiff(true);
  }, [onInspectDiff]);

  // Virtualized row renderer — only used for Smart Collapse (clusters have uniform height)
  const renderClusterRow = useCallback(({ index, style }: ListChildComponentProps) => {
    const cluster = compressed?.clusters?.[index];
    if (!cluster) return null;
    return (
      <div style={style} key={cluster.pattern} className="cluster-line">
        <span className="cluster-count">×{cluster.count}</span>
        <span className="cluster-pattern">{cluster.pattern}</span>
        <span className="cluster-ago">{timeAgo(cluster.last_seen_ms)}</span>
      </div>
    );
  }, [compressed]);

  // Plain item renderer — no fixed height constraint
  const renderItem = useCallback((item: DisplayItem) => {
    if (item.kind === 'dedup') {
      return (
        <div key={item.id} className="dedup-line" title={`${item.count} identical messages collapsed`}>
          <span className="dedup-badge">×{item.count}</span>
          <span className="stream-time">{timeLabel(item.representative.timestamp_ms)}</span>
          <span className={`stream-source-tag ${item.representative.source_type}`}>{item.representative.source_label}</span>
          <span className="stream-text" style={{ color: 'var(--text-secondary)' }}>{item.representative.text}</span>
        </div>
      );
    }

    if (item.kind === 'stack') {
      return (
        <div key={item.id} className="stack-trace-block" style={{ margin: '0.25rem 0.75rem' }}>
          <div className="stack-trace-header">
            <span className={`stack-trace-lang ${item.lang}`}>{item.lang}</span>
            <span className="stack-trace-message">{item.header}</span>
          </div>
          <div className="stack-trace-frames">
            {item.frames.slice(0, 5).map((f, i) => (
              <div key={i} className={`stack-frame${f.includes('src/') ? ' user-code' : ''}`}>{f.trim()}</div>
            ))}
            {item.frames.length > 5 && (
              <div className="stack-frame muted">…+{item.frames.length - 5} more</div>
            )}
          </div>
          <button className="inspect-btn" onClick={handleInspectDiff}>
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
            Inspect Changes
          </button>
        </div>
      );
    }

    const entry = item.entry;
    const lvl = entry.level ?? '';
    const levelTag = lvl === 'fatal' || lvl === 'error'
      ? <span className="level-badge error">ERR</span>
      : lvl === 'warn'
      ? <span className="level-badge warn">WRN</span>
      : null;

    return (
      <div
        key={entry.id}
        className={`stream-line${lvl === 'fatal' || lvl === 'error' ? ' has-error' : lvl === 'warn' ? ' has-warn' : ''}`}
        onClick={() => onCorrelate(entry)}
        title="Click to inspect correlations"
        role="button"
        tabIndex={0}
        onKeyDown={e => e.key === 'Enter' && onCorrelate(entry)}
      >
        <span className="stream-time">{timeLabel(entry.timestamp_ms)}</span>
        <span className={`stream-source-tag ${entry.source_type}`}>{entry.source_label}</span>
        {levelTag}
        <span className="stream-text">{entry.text}</span>
      </div>
    );
  }, [onCorrelate, handleInspectDiff]);

  return (
    <div className="stream-root">
      {/* Toolbar */}
      <div className="stream-toolbar">
        {onBack && (
          <button className="triage-back-btn" onClick={onBack} style={{ marginRight: '0.25rem' }}>
            <ArrowLeft size={12} /> Overview
          </button>
        )}
        <div className="tab-row">
          {(['all', 'terminal', 'docker', 'network', 'compressed'] as TabKind[]).map(t => (
            <button
              key={t}
              className={`tab-btn${tab === t ? ' active' : ''}`}
              onClick={() => setTab(t)}
            >
              {t === 'all' && 'All'}
              {t === 'terminal' && 'Terminal'}
              {t === 'docker' && 'Docker'}
              {t === 'network' && 'Network'}
              {t === 'compressed' && 'Analyzed'}
            </button>
          ))}
        </div>

        <div className="spacer" />

        <div className="input-row" style={{ minWidth: 200, maxWidth: 280 }}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ flexShrink: 0, color: 'var(--text-muted)' }}>
            <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          <input
            placeholder="filter  or  span_id: abc"
            value={filter}
            onChange={e => setFilter(e.target.value)}
            spellCheck={false}
          />
          {filter && (
            <button style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 0, fontSize: '0.8rem' }} onClick={() => setFilter('')}>✕</button>
          )}
        </div>

        {(tab === 'all' || tab === 'terminal') && (
          <button
            className={`collapse-toggle${smartCollapse ? ' active' : ''}`}
            onClick={() => setSmartCollapse(s => !s)}
            title="Smart Collapse — group repeated messages via Drain"
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <path d="M3 6h18M7 12h10M11 18h2" />
            </svg>
            {smartCollapse ? 'expanded' : 'collapse'}
          </button>
        )}


      </div>

      {/* Sub-bar: count + filters + auto-scroll */}
      <div className="stream-subbar">
        <span className="stream-meta">
          {smartCollapse
            ? `${compressed?.clusters?.length ?? 0} clusters`
            : `${displayItems.length} entries`
          }
          {selectedSource && ` · ${selectedSource}`}
          {timeFilter !== null && ` · minute filter`}
          {filter && ` · "${filter}"`}
        </span>
        <div className="spacer" />
        <button
          className={`collapse-toggle${autoScroll ? ' active' : ''}`}
          style={{ fontSize: '0.55rem', padding: '0.15rem 0.4rem' }}
          onClick={() => setAutoScroll(a => !a)}
        >
          auto-scroll {autoScroll ? 'on' : 'off'}
        </button>
      </div>

      {/* Main content area */}
      <div style={{ flex: 1, minHeight: 0, position: 'relative' }}>
        {tab === 'network' ? (
          <NetworkInspector httpErrors={httpErrors} />
        ) : tab === 'compressed' ? (
          <div className="stream-list custom-scrollbar" style={{ height: '100%', overflowY: 'auto' }}>
            {compressed?.stack_traces?.map((trace, i) => (
              <div key={i} className="stack-trace-block" style={{ margin: '0.5rem 0.75rem' }}>
                <div className="stack-trace-header">
                  <span className={`stack-trace-lang ${trace.language}`}>{trace.language}</span>
                  <span className="stack-trace-message">{trace.error_message}</span>
                </div>
                <div className="stack-trace-frames">
                  {trace.frames.slice(0, 5).map((f, j) => (
                    <div key={j} className={`stack-frame${f.is_user_code ? ' user-code' : ''}`}>{f.raw}</div>
                  ))}
                  {trace.frames.length > 5 && (
                    <div className="stack-frame muted">…+{trace.frames.length - 5} more</div>
                  )}
                </div>
                <button className="inspect-btn" onClick={handleInspectDiff}>
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                  Inspect Changes
                </button>
              </div>
            ))}
            {compressed?.clusters?.map((c, i) => (
              <div key={i} className="cluster-line">
                <span className="cluster-count">×{c.count}</span>
                <span className="cluster-pattern">{c.pattern}</span>
                <span className="cluster-ago">{timeAgo(c.last_seen_ms)}</span>
              </div>
            ))}
            {!compressed?.clusters?.length && !compressed?.stack_traces?.length && (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.75rem' }}>
                No error clusters or stack traces detected
              </div>
            )}
          </div>
        ) : smartCollapse && compressed?.clusters?.length ? (
          /* Smart Collapse: uniform-height clusters — safe to virtualize */
          <AutoSizer>
            {({ height, width }: { height: number; width: number }) => (
              <List
                ref={listRef}
                height={height}
                width={width}
                itemCount={compressed.clusters.length}
                itemSize={28}
                overscanCount={20}
                className="custom-scrollbar"
                onScroll={({ scrollUpdateWasRequested }: { scrollUpdateWasRequested: boolean }) => {
                  if (!scrollUpdateWasRequested) setAutoScroll(false);
                }}
              >
                {renderClusterRow}
              </List>
            )}
          </AutoSizer>
        ) : displayItems.length === 0 ? (
          <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.75rem', fontFamily: "'JetBrains Mono', monospace" }}>
            {filter ? `No entries matching "${filter}"` : 'No log entries'}
          </div>
        ) : (
          /* Plain scrollable list — supports variable-height stack trace blocks */
          <div
            ref={scrollRef}
            className="stream-list custom-scrollbar"
            style={{ height: '100%', overflowY: 'auto' }}
            onScroll={handleScroll}
          >
            {displayItems.map(item => renderItem(item))}
          </div>
        )}

        {showDiff && diff && (
          <div className="diff-overlay">
            <div className="card-header">
              <span className="card-title">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
                Surgical Diff
              </span>
              <div className="spacer" />
              <button className="btn btn-ghost" style={{ padding: '0.2rem 0.5rem', minHeight: 28 }} onClick={() => setShowDiff(false)}>✕</button>
            </div>
            <div style={{ flex: 1, overflow: 'hidden' }}>
              <GitLens diff={diff} onRefresh={onInspectDiff} loading={false} />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
