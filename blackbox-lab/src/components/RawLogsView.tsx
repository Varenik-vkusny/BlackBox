import { useState, useMemo, useRef, useEffect } from 'react';
import { ArrowLeft, Terminal, Box, Globe } from 'lucide-react';
import type { LogLine, DockerEvent, HttpEvent, DockerResponse, HttpErrorsResponse } from '../types';

interface Props {
  logLines: LogLine[];
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  onBack: () => void;
}

/* ═══════════════════════════════════════════════════
   Helpers
   ═══════════════════════════════════════════════════ */

function timeLabel(ms: number): string {
  return new Date(ms).toTimeString().slice(0, 8);
}

function resolveContainerName(id: string, docker: DockerResponse | null): string {
  const short = id.slice(0, 12);
  const match = docker?.containers.find(c => c.includes(short) || short.includes(c.slice(0, 8)));
  return match ?? short;
}

/* ═══════════════════════════════════════════════════
   Column Header
   ═══════════════════════════════════════════════════ */

interface ColumnHeaderProps {
  title: string;
  icon: React.ReactNode;
  count: number;
  total: number;
  filter: string;
  onFilterChange: (v: string) => void;
}

function ColumnHeader({ title, icon, count, total, filter, onFilterChange }: ColumnHeaderProps) {
  const isRegex = filter.startsWith('/');
  return (
    <div className="rl-col-header">
      <div className="rl-col-title-row">
        <span style={{ color: 'var(--fg-muted)', display: 'flex' }}>{icon}</span>
        <span className="rl-col-title">{title}</span>
        <span className="rl-col-count">{filter ? `${count} / ${total}` : total}</span>
      </div>
      <div className="rl-col-filter">
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ color: 'var(--fg-muted)', flexShrink: 0 }}>
          <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <input
          placeholder="filter…"
          value={filter}
          onChange={e => onFilterChange(e.target.value)}
          spellCheck={false}
          style={{ fontFamily: "'Geist Mono', monospace" }}
        />
        {isRegex && (
          <span style={{ fontSize: '10px', color: 'var(--brand)', fontWeight: 600, fontFamily: "'Geist Mono', monospace", flexShrink: 0 }}>R</span>
        )}
        {filter && (
          <button onClick={() => onFilterChange('')} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--fg-muted)', padding: 0, fontSize: '0.75rem', lineHeight: 1 }}>✕</button>
        )}
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Unified Row
   ═══════════════════════════════════════════════════ */

interface RowData {
  timestamp_ms: number;
  terminal: LogLine[];
  docker: DockerEvent[];
  http: HttpEvent[];
}

function fallbackTs(ms?: number): number {
  return (ms && ms > 0) ? ms : Math.floor(Date.now() / 1000) * 1000;
}

function makeRows(
  terminalLines: LogLine[],
  dockerEvents: DockerEvent[],
  httpEvents: HttpEvent[],
): RowData[] {
  const byTime = new Map<number, RowData>();

  for (const line of terminalLines) {
    const t = fallbackTs(line.timestamp_ms);
    if (!byTime.has(t)) byTime.set(t, { timestamp_ms: t, terminal: [], docker: [], http: [] });
    byTime.get(t)!.terminal.push(line);
  }

  for (const ev of dockerEvents) {
    const t = fallbackTs(ev.timestamp_ms);
    if (!byTime.has(t)) byTime.set(t, { timestamp_ms: t, terminal: [], docker: [], http: [] });
    byTime.get(t)!.docker.push(ev);
  }

  for (const ev of httpEvents) {
    const t = fallbackTs(ev.timestamp_ms);
    if (!byTime.has(t)) byTime.set(t, { timestamp_ms: t, terminal: [], docker: [], http: [] });
    byTime.get(t)!.http.push(ev);
  }

  const rows = Array.from(byTime.values());
  rows.sort((a, b) => a.timestamp_ms - b.timestamp_ms);
  return rows;
}

/* ═══════════════════════════════════════════════════
   Cell renderers
   ═══════════════════════════════════════════════════ */

function TerminalCell({ lines }: { lines: LogLine[] }) {
  if (lines.length === 0) return null;
  return (
    <div className="rl-cell-terminal">
      {lines.map((line, i) => {
        const text = line.text;
        const lvl = /\b(fatal|panic)\b/i.test(text) ? 'fatal'
          : /\berror\b/i.test(text) ? 'error'
          : /\bwarn(ing)?\b/i.test(text) ? 'warn'
          : null;
        return (
          <div key={i} className={`rl-cell-line${lvl ? ` rl-lvl-${lvl}` : ''}`}>
            {line.source_terminal && line.source_terminal !== '' && (
              <span className="rl-cell-source">{line.source_terminal}</span>
            )}
            <span style={{ color: lvl === 'fatal' || lvl === 'error' ? 'var(--severity-error)' : lvl === 'warn' ? 'var(--severity-warn)' : 'var(--fg-secondary)' }}>{text}</span>
          </div>
        );
      })}
    </div>
  );
}

function DockerCell({ events, docker }: { events: DockerEvent[]; docker: DockerResponse | null }) {
  if (events.length === 0) return <div className="rl-cell-empty">·</div>;
  return (
    <div className="rl-cell-docker">
      {events.map((ev, i) => {
        const lvl = (ev.level?.toLowerCase() ?? null) as 'error' | 'warn' | null;
        const cname = resolveContainerName(ev.source.container_id, docker);
        return (
          <div key={i} className={`rl-cell-line${lvl === 'error' ? ' rl-lvl-error' : ''}`}>
            <span className="rl-cell-source">{cname}</span>
            <span style={{ color: lvl === 'error' ? 'var(--severity-error)' : 'var(--fg-secondary)' }}>{ev.text}</span>
          </div>
        );
      })}
    </div>
  );
}

function HttpCell({ events }: { events: HttpEvent[] }) {
  if (events.length === 0) return <div className="rl-cell-empty">·</div>;
  return (
    <div className="rl-cell-http">
      {events.map((ev, i) => {
        const is5xx = ev.status >= 500;
        const statusColor = is5xx ? 'var(--severity-error)' : 'var(--severity-warn)';
        return (
          <div key={i} className={`rl-cell-line${is5xx ? ' rl-lvl-error' : ' rl-lvl-warn'}`}>
            <span className="rl-cell-source">{ev.method}</span>
            <span style={{ color: statusColor, fontWeight: 600 }}>{ev.status}</span>
            <span className="rl-cell-url">{ev.url}</span>
            <span className="rl-cell-latency">{ev.latency_ms}ms</span>
          </div>
        );
      })}
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Root
   ═══════════════════════════════════════════════════ */

export function RawLogsView({ logLines, docker, httpErrors, onBack }: Props) {
  const [terminalFilter, setTerminalFilter] = useState('');
  const [dockerFilter, setDockerFilter] = useState('');
  const [httpFilter, setHttpFilter] = useState('');
  const [autoScroll, setAutoScroll] = useState(true);

  const scrollRef = useRef<HTMLDivElement>(null);

  // Filter terminal lines
  const filteredTerminal = useMemo(() => {
    if (!terminalFilter) return logLines;
    const q = terminalFilter.toLowerCase();
    return logLines.filter(l => l.text.toLowerCase().includes(q));
  }, [logLines, terminalFilter]);

  // Filter docker events
  const filteredDocker = useMemo(() => {
    const events = docker?.events ?? [];
    if (!dockerFilter) return events;
    const q = dockerFilter.toLowerCase();
    return events.filter(ev =>
      ev.text.toLowerCase().includes(q) ||
      ev.source.container_id.toLowerCase().includes(q)
    );
  }, [docker, dockerFilter]);

  // Filter HTTP events
  const filteredHttp = useMemo(() => {
    const events = httpErrors?.events ?? [];
    if (!httpFilter) return events;
    const q = httpFilter.toLowerCase();
    return events.filter(ev =>
      ev.url.toLowerCase().includes(q) ||
      String(ev.status).includes(q) ||
      ev.method.toLowerCase().includes(q)
    );
  }, [httpErrors, httpFilter]);

  const rows = useMemo(() => makeRows(filteredTerminal, filteredDocker, filteredHttp), [filteredTerminal, filteredDocker, filteredHttp]);

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [rows.length, autoScroll]);

  const totalEntries = logLines.length + (docker?.events.length ?? 0) + (httpErrors?.events.length ?? 0);

  return (
    <div className="raw-logs-view">
      <div className="raw-logs-header">
        <button className="triage-back-btn" onClick={onBack}>
          <ArrowLeft size={12} /> Overview
        </button>
        <span className="raw-logs-title">Raw Logs</span>
        <div style={{ flex: 1 }} />
        <span className="raw-logs-subtitle">{totalEntries} total entries · {rows.length} unique timestamps</span>
      </div>

      <div className="rl-grid">
        {/* Sticky column headers */}
        <div className="rl-grid-header">
          <div className="rl-time-col">
            <span className="rl-col-title">Time</span>
          </div>
          <div className="rl-data-col">
            <ColumnHeader
              title="Terminal"
              icon={<Terminal size={13} />}
              count={filteredTerminal.length}
              total={logLines.length}
              filter={terminalFilter}
              onFilterChange={setTerminalFilter}
            />
          </div>
          <div className="rl-data-col">
            <ColumnHeader
              title="Docker"
              icon={<Box size={13} />}
              count={filteredDocker.length}
              total={docker?.events.length ?? 0}
              filter={dockerFilter}
              onFilterChange={setDockerFilter}
            />
          </div>
          <div className="rl-data-col">
            <ColumnHeader
              title="HTTP"
              icon={<Globe size={13} />}
              count={filteredHttp.length}
              total={httpErrors?.events.length ?? 0}
              filter={httpFilter}
              onFilterChange={setHttpFilter}
            />
          </div>
        </div>

        {/* Scrollable body */}
        <div
          ref={scrollRef}
          className="rl-grid-body custom-scrollbar"
          onScroll={e => {
            const el = e.currentTarget;
            setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 60);
          }}
        >
          {rows.length === 0 && (
            <div className="rl-empty">
              {terminalFilter || dockerFilter || httpFilter
                ? 'No events match the current filters'
                : 'No events captured yet'}
            </div>
          )}

          {rows.map(row => (
            <div key={row.timestamp_ms} className="rl-row">
              <div className="rl-time-col">
                <span className="rl-time-label">{timeLabel(row.timestamp_ms)}</span>
              </div>
              <div className="rl-data-col">
                <TerminalCell lines={row.terminal} />
              </div>
              <div className="rl-data-col">
                <DockerCell events={row.docker} docker={docker} />
              </div>
              <div className="rl-data-col">
                <HttpCell events={row.http} />
              </div>
            </div>
          ))}
        </div>

        <div className="rl-footer">
          <button
            className={`collapse-toggle${autoScroll ? ' active' : ''}`}
            style={{ fontSize: '11px', padding: '2px 6px' }}
            onClick={() => setAutoScroll(a => !a)}
          >
            auto-scroll <span className={autoScroll ? 'toggle-on' : ''}>{autoScroll ? 'on' : 'off'}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
