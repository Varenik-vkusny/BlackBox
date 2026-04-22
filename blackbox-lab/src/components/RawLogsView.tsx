import { useState, useRef, useEffect, useMemo } from 'react';
import { ArrowLeft, Terminal, Box, Globe } from 'lucide-react';
import type { LogLine, DockerResponse, HttpErrorsResponse } from '../types';

interface Props {
  logLines: LogLine[];
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  onBack: () => void;
}

// ── Helpers ───────────────────────────────────────────────────

function levelOf(text: string): 'fatal' | 'error' | 'warn' | null {
  const t = text.toLowerCase();
  if (/\b(fatal|panic)\b/.test(t)) return 'fatal';
  if (/\berror\b/.test(t)) return 'error';
  if (/\bwarn(ing)?\b/.test(t)) return 'warn';
  return null;
}

function levelTextColor(lvl: string | null): string {
  if (lvl === 'fatal') return 'var(--accent-red)';
  if (lvl === 'error') return '#f87171';
  if (lvl === 'warn') return 'var(--accent-orange)';
  return 'var(--text-secondary)';
}

function timeLabel(ms: number): string {
  return new Date(ms).toTimeString().slice(0, 8);
}

const CONTAINER_PALETTE = [
  '#22d3ee', '#a78bfa', '#34d399', '#fb923c', '#f472b6', '#60a5fa',
];

function containerColor(name: string): string {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = name.charCodeAt(i) + ((h << 5) - h);
  return CONTAINER_PALETTE[Math.abs(h) % CONTAINER_PALETTE.length];
}

const METHOD_COLORS: Record<string, string> = {
  GET: '#34d399', POST: '#60a5fa', PUT: '#a78bfa',
  PATCH: '#fb923c', DELETE: '#f87171',
};

// ── Reusable pane shell ───────────────────────────────────────

interface PaneProps {
  title: string;
  icon: React.ReactNode;
  count: number;
  total: number;
  accentColor: string;
  filter: string;
  onFilterChange: (v: string) => void;
  children: React.ReactNode;
  isEmpty: boolean;
  emptyMessage: string;
  triggerScroll?: number; // increment to trigger auto-scroll
}

function LogPane({
  title, icon, count, total, accentColor,
  filter, onFilterChange, children, isEmpty, emptyMessage, triggerScroll,
}: PaneProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [triggerScroll, autoScroll]);

  return (
    <div className="log-pane" style={{ '--pane-accent': accentColor } as React.CSSProperties}>
      <div className="log-pane-header">
        <div className="log-pane-title-row">
          <span style={{ color: accentColor, display: 'flex' }}>{icon}</span>
          <span className="log-pane-title">{title}</span>
          <span className="log-pane-count">{filter ? `${count} / ${total}` : total}</span>
        </div>
        <div className="log-pane-filter">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ color: 'var(--text-muted)', flexShrink: 0 }}>
            <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          <input
            placeholder="filter…"
            value={filter}
            onChange={e => onFilterChange(e.target.value)}
            spellCheck={false}
          />
          {filter && (
            <button onClick={() => onFilterChange('')} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 0, fontSize: '0.75rem', lineHeight: 1 }}>✕</button>
          )}
        </div>
      </div>

      <div
        ref={scrollRef}
        className="log-pane-body custom-scrollbar"
        onScroll={e => {
          const el = e.currentTarget;
          setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 60);
        }}
      >
        {isEmpty ? (
          <div className="log-pane-empty">{emptyMessage}</div>
        ) : children}
      </div>

      <div className="log-pane-footer">
        <button
          className={`collapse-toggle${autoScroll ? ' active' : ''}`}
          style={{ fontSize: '0.5rem', padding: '0.1rem 0.35rem' }}
          onClick={() => setAutoScroll(a => !a)}
        >
          auto-scroll {autoScroll ? 'on' : 'off'}
        </button>
      </div>
    </div>
  );
}

// ── Terminal pane (one per shell session) ─────────────────────

function TerminalSessionPane({ sessionName, lines, filter, onFilterChange }: {
  sessionName: string;
  lines: LogLine[];
  filter: string;
  onFilterChange: (v: string) => void;
}) {
  const filtered = useMemo(() => {
    if (!filter) return lines;
    const q = filter.toLowerCase();
    return lines.filter(l => l.text.toLowerCase().includes(q));
  }, [lines, filter]);

  const label = sessionName || 'injected';
  const isInjected = !sessionName;

  return (
    <LogPane
      title={`Terminal · ${label}`}
      icon={<Terminal size={13} />}
      count={filtered.length}
      total={lines.length}
      accentColor={isInjected ? 'var(--accent-green)' : 'var(--accent-cyan)'}
      filter={filter}
      onFilterChange={onFilterChange}
      isEmpty={filtered.length === 0}
      emptyMessage={filter ? `No lines matching "${filter}"` : 'No output captured yet'}
      triggerScroll={lines.length}
    >
      {filtered.map((line, i) => {
        const lvl = levelOf(line.text);
        return (
          <div key={i} className={`log-pane-line${lvl ? ` lvl-${lvl}` : ''}`}>
            <span className="lp-time">{timeLabel(line.timestamp_ms)}</span>
            <span className="lp-text" style={{ color: levelTextColor(lvl) }}>{line.text}</span>
          </div>
        );
      })}
    </LogPane>
  );
}

// ── Docker pane ───────────────────────────────────────────────

function DockerPane({ docker, filter, onFilterChange }: {
  docker: DockerResponse | null;
  filter: string;
  onFilterChange: (v: string) => void;
}) {
  const events = docker?.events ?? [];
  const offline = docker != null && !docker.docker_available;

  const filtered = useMemo(() => {
    if (!filter) return events;
    const q = filter.toLowerCase();
    return events.filter(ev =>
      ev.text.toLowerCase().includes(q) ||
      ev.source.container_id.toLowerCase().includes(q)
    );
  }, [events, filter]);

  function resolveContainerName(id: string): string {
    const short = id.slice(0, 12);
    const match = docker?.containers.find(c => c.includes(short) || short.includes(c.slice(0, 8)));
    return match ?? short;
  }

  return (
    <LogPane
      title="Docker"
      icon={<Box size={13} />}
      count={filtered.length}
      total={events.length}
      accentColor="#60a5fa"
      filter={filter}
      onFilterChange={onFilterChange}
      isEmpty={offline || filtered.length === 0}
      emptyMessage={
        offline
          ? 'Docker daemon not reachable'
          : filter
          ? `No events matching "${filter}"`
          : 'No Docker events captured'
      }
      triggerScroll={events.length}
    >
      {filtered.map((ev, i) => {
        const lvl = (ev.level?.toLowerCase() ?? null) as 'error' | 'warn' | null;
        const cname = resolveContainerName(ev.source.container_id);
        const ccolor = containerColor(cname);
        return (
          <div key={i} className={`log-pane-line${lvl === 'error' ? ' lvl-error' : lvl === 'warn' ? ' lvl-warn' : ''}`}>
            <span className="lp-time">{timeLabel(ev.timestamp_ms)}</span>
            <span className="lp-container" style={{ color: ccolor, borderColor: `${ccolor}35` }}>
              {cname}
            </span>
            <span className="lp-text" style={{ color: levelTextColor(lvl) }}>{ev.text}</span>
          </div>
        );
      })}
    </LogPane>
  );
}

// ── HTTP pane ─────────────────────────────────────────────────

function HttpPane({ httpErrors, filter, onFilterChange }: {
  httpErrors: HttpErrorsResponse | null;
  filter: string;
  onFilterChange: (v: string) => void;
}) {
  const events = httpErrors?.events ?? [];

  const filtered = useMemo(() => {
    if (!filter) return events;
    const q = filter.toLowerCase();
    return events.filter(ev =>
      ev.url.toLowerCase().includes(q) ||
      String(ev.status).includes(q) ||
      ev.method.toLowerCase().includes(q)
    );
  }, [events, filter]);

  return (
    <LogPane
      title="HTTP"
      icon={<Globe size={13} />}
      count={filtered.length}
      total={events.length}
      accentColor="#fb923c"
      filter={filter}
      onFilterChange={onFilterChange}
      isEmpty={filtered.length === 0}
      emptyMessage={
        filter
          ? `No requests matching "${filter}"`
          : 'No HTTP errors captured\n4xx/5xx only · route via proxy :8769'
      }
      triggerScroll={events.length}
    >
      {filtered.map((ev, i) => {
        const is5xx = ev.status >= 500;
        const statusColor = is5xx ? 'var(--accent-red)' : 'var(--accent-orange)';
        const methodColor = METHOD_COLORS[ev.method] ?? 'var(--text-muted)';
        return (
          <div key={i} className={`log-pane-line${is5xx ? ' lvl-error' : ' lvl-warn'}`}>
            <span className="lp-time">{timeLabel(ev.timestamp_ms)}</span>
            <span className="lp-method" style={{ color: methodColor }}>{ev.method}</span>
            <span className="lp-status" style={{ color: statusColor }}>{ev.status}</span>
            <span className="lp-url">{ev.url}</span>
            <span className="lp-latency">{ev.latency_ms}ms</span>
          </div>
        );
      })}
    </LogPane>
  );
}

// ── Root ──────────────────────────────────────────────────────

export function RawLogsView({ logLines, docker, httpErrors, onBack }: Props) {
  // Group terminal lines by session name. Map preserves insertion order so
  // sessions appear in the order their first line was captured.
  const terminalGroups = useMemo(() => {
    const groups = new Map<string, LogLine[]>();
    for (const line of logLines) {
      const key = line.source_terminal ?? '';
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(line);
    }
    return groups;
  }, [logLines]);

  // Per-session filter state keyed by session name
  const [sessionFilters, setSessionFilters] = useState<Record<string, string>>({});
  const [dockerFilter, setDockerFilter] = useState('');
  const [httpFilter, setHttpFilter] = useState('');

  function setSessionFilter(key: string, value: string) {
    setSessionFilters(prev => ({ ...prev, [key]: value }));
  }

  const totalEntries =
    logLines.length +
    (docker?.events.length ?? 0) +
    (httpErrors?.events.length ?? 0);

  return (
    <div className="raw-logs-view">
      <div className="raw-logs-header">
        <button className="triage-back-btn" onClick={onBack}>
          <ArrowLeft size={12} /> Overview
        </button>
        <span className="raw-logs-title">Raw Logs</span>
        <div style={{ flex: 1 }} />
        <span className="raw-logs-subtitle">{totalEntries} total entries · {terminalGroups.size} terminal session{terminalGroups.size !== 1 ? 's' : ''}</span>
      </div>

      <div className="raw-logs-panes">
        {Array.from(terminalGroups.entries()).map(([key, lines]) => (
          <TerminalSessionPane
            key={key || '__injected__'}
            sessionName={key}
            lines={lines}
            filter={sessionFilters[key] ?? ''}
            onFilterChange={v => setSessionFilter(key, v)}
          />
        ))}
        <DockerPane docker={docker} filter={dockerFilter} onFilterChange={setDockerFilter} />
        <HttpPane httpErrors={httpErrors} filter={httpFilter} onFilterChange={setHttpFilter} />
      </div>
    </div>
  );
}
