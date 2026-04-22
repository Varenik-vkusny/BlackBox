import { useState } from 'react';
import type { HttpErrorsResponse, HttpEvent } from '../types';

interface Props {
  httpErrors: HttpErrorsResponse | null;
}

function timeLabel(ms: number): string {
  return new Date(ms).toTimeString().slice(0, 8);
}

function MethodBadge({ method }: { method: string }) {
  const colors: Record<string, string> = {
    GET:    'rgba(34,211,238,0.15)',
    POST:   'rgba(34,197,94,0.15)',
    PUT:    'rgba(234,179,8,0.15)',
    PATCH:  'rgba(249,115,22,0.15)',
    DELETE: 'rgba(244,63,94,0.15)',
  };
  return (
    <span
      className="http-method"
      style={{ background: colors[method] ?? 'rgba(148,163,184,0.1)', color: 'var(--text-secondary)' }}
    >
      {method}
    </span>
  );
}

function StatusCell({ status }: { status: number }) {
  const cls = status >= 500 ? 'http-status-5xx' : 'http-status-4xx';
  return <span className={cls}>{status}</span>;
}

function ExpandedRow({ event }: { event: HttpEvent }) {
  return (
    <tr className="network-row-detail">
      <td colSpan={5}>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
          <div>
            <div style={{ fontSize: '0.58rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', color: 'var(--text-muted)', marginBottom: '0.35rem' }}>
              Request Body
            </div>
            <div className="network-detail-body">
              {event.request_body ?? <span style={{ color: 'var(--text-muted)' }}>(empty)</span>}
            </div>
          </div>
          <div>
            <div style={{ fontSize: '0.58rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', color: 'var(--text-muted)', marginBottom: '0.35rem' }}>
              Response Body
            </div>
            <div className="network-detail-body">
              {event.response_body ?? <span style={{ color: 'var(--text-muted)' }}>(empty)</span>}
            </div>
          </div>
        </div>
      </td>
    </tr>
  );
}

export function NetworkInspector({ httpErrors }: Props) {
  const [expanded, setExpanded] = useState<string | null>(null);

  if (!httpErrors?.events?.length) {
    return (
      <div className="network-empty">
        <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.75rem' }}>
          No HTTP errors captured
        </div>
        <div style={{ color: 'var(--text-muted)', lineHeight: 2, fontFamily: "'JetBrains Mono', monospace" }}>
          Route traffic through the BlackBox proxy:<br />
          <span style={{ color: 'var(--accent-cyan)' }}>HTTP_PROXY=http://127.0.0.1:{httpErrors?.proxy_port ?? 8769}</span><br />
          Only 4xx / 5xx responses are stored.
        </div>
      </div>
    );
  }

  return (
    <div className="custom-scrollbar" style={{ height: '100%', overflowY: 'auto', overflowX: 'auto' }}>
      <table className="network-table">
        <thead>
          <tr>
            <th>Method</th>
            <th>URL</th>
            <th>Status</th>
            <th>Latency</th>
            <th>Time</th>
          </tr>
        </thead>
        <tbody>
          {httpErrors.events.map((ev, i) => {
            const key = `${ev.timestamp_ms}-${i}`;
            const isExpanded = expanded === key;
            const urlShort = ev.url.length > 60 ? ev.url.slice(0, 60) + '…' : ev.url;
            return (
              <>
                <tr
                  key={key}
                  className={`network-row${isExpanded ? ' expanded' : ''}`}
                  onClick={() => setExpanded(isExpanded ? null : key)}
                >
                  <td><MethodBadge method={ev.method} /></td>
                  <td style={{ maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', color: 'var(--text-secondary)' }}>
                    {urlShort}
                  </td>
                  <td><StatusCell status={ev.status} /></td>
                  <td style={{ color: ev.latency_ms > 1000 ? 'var(--accent-orange)' : 'var(--text-muted)' }}>
                    {ev.latency_ms}ms
                  </td>
                  <td style={{ color: 'var(--text-muted)', whiteSpace: 'nowrap' }}>
                    {timeLabel(ev.timestamp_ms)}
                  </td>
                </tr>
                {isExpanded && <ExpandedRow key={`${key}-detail`} event={ev} />}
              </>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
