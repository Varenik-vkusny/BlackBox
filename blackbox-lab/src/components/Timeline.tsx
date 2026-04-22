import { useRef, useState, useCallback } from 'react';
import type { BBStatus, PostmortemResponse, DockerResponse, HttpErrorsResponse } from '../types';

interface Props {
  postmortem: PostmortemResponse | null;
  status: BBStatus | null;
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  daemonOnline: boolean;
  timeFilter: number | null;
  onTimeFilter: (minute: number | null) => void;
}

interface TooltipState {
  x: number;
  y: number;
  text: string;
}

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

function msAgo(ms: number): string {
  const d = Date.now() - ms;
  if (d < 60000) return `${Math.floor(d / 1000)}s ago`;
  return `${Math.floor(d / 60000)}m ago`;
}

export function Timeline({ postmortem, status, docker, httpErrors, daemonOnline, timeFilter, onTimeFilter }: Props) {
  const chartRef = useRef<SVGSVGElement>(null);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  const buckets = postmortem?.timeline ?? [];
  const maxErrors = Math.max(1, ...buckets.map(b => b.error_count));
  const maxLines  = Math.max(1, ...buckets.map(b => b.line_count));

  const BAR_H = 64;
  const BAR_GAP = 1;

  const handleMouseMove = useCallback((e: React.MouseEvent<SVGRectElement>, i: number) => {
    const b = buckets[i];
    if (!b) return;
    const rect = chartRef.current?.getBoundingClientRect();
    if (!rect) return;
    setTooltip({
      x: e.clientX - rect.left,
      y: e.clientY - rect.top - 36,
      text: `−${buckets.length - 1 - i}m  •  ${b.error_count} errors  •  ${b.line_count} lines`,
    });
  }, [buckets]);

  const handleClick = useCallback((i: number) => {
    const offset = buckets[i]?.minute_offset ?? null;
    onTimeFilter(timeFilter === offset ? null : offset);
  }, [buckets, timeFilter, onTimeFilter]);

  // Source availability badges
  const dockerOk = docker?.docker_available;
  const httpCount = httpErrors?.total ?? 0;
  const http4xx = httpErrors?.events?.filter(e => e.status < 500).length ?? 0;
  const http5xx = httpErrors?.events?.filter(e => e.status >= 500).length ?? 0;

  return (
    <div className="timeline-root">
      {/* Histogram */}
      <div className="timeline-chart" style={{ position: 'relative' }}>
        <svg
          ref={chartRef}
          onMouseLeave={() => setTooltip(null)}
          aria-label="Error timeline — click a bar to filter by minute"
        >
          {buckets.map((b, i) => {
            const total = buckets.length || 1;
            const w = `${(100 / total).toFixed(2)}%`;
            const xPct = (i / total * 100).toFixed(2) + '%';

            // Height scaled by error activity; low-level activity shown as volume
            const errorFrac = b.error_count / maxErrors;
            const lineFrac  = Math.min(b.line_count / maxLines, 1);
            const h = Math.max(3, Math.round(errorFrac > 0 ? errorFrac * BAR_H : lineFrac * BAR_H * 0.3));
            const y = BAR_H - h;

            const fill = b.error_count > 0
              ? `rgba(244,63,94,${0.4 + errorFrac * 0.6})`
              : `rgba(34,197,94,0.25)`;

            const isActive = timeFilter === b.minute_offset;

            return (
              <rect
                key={i}
                className={`timeline-bar${isActive ? ' active' : ''}`}
                x={xPct}
                y={y}
                width={`calc(${w} - ${BAR_GAP}px)`}
                height={h}
                fill={fill}
                rx={2}
                onMouseMove={e => handleMouseMove(e, i)}
                onClick={() => handleClick(i)}
                role="button"
                tabIndex={0}
                aria-label={`Minute -${buckets.length - 1 - i}: ${b.error_count} errors`}
                onKeyDown={e => e.key === 'Enter' && handleClick(i)}
              />
            );
          })}
        </svg>

        {/* Label strip */}
        <div className="timeline-label-row">
          {buckets.length === 0 && (
            <span className="timeline-label" style={{ textAlign: 'left' }}>No postmortem data</span>
          )}
          {buckets.length > 0 && (
            <>
              <span className="timeline-label" style={{ textAlign: 'left' }}>
                −{buckets.length - 1}m
              </span>
              <span className="timeline-label">now</span>
            </>
          )}
        </div>

        {tooltip && (
          <div
            className="timeline-tooltip"
            style={{ left: Math.min(tooltip.x, 320), top: tooltip.y }}
          >
            {tooltip.text}
          </div>
        )}

        {timeFilter !== null && (
          <button
            className="btn btn-ghost"
            style={{ position: 'absolute', top: 4, right: 4, padding: '0.2rem 0.5rem', fontSize: '0.6rem', minHeight: 24 }}
            onClick={() => onTimeFilter(null)}
            title="Clear time filter"
          >
            ✕ clear filter
          </button>
        )}
      </div>

      {/* Status strip */}
      <div className="timeline-status">
        {/* Daemon status */}
        <div className="timeline-status-row">
          <span
            className={`source-badge ${daemonOnline ? (status?.has_recent_errors ? 'warn' : 'online') : 'offline'}`}
          >
            <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'currentColor', display: 'inline-block' }} />
            {daemonOnline ? (status?.has_recent_errors ? 'errors' : 'nominal') : 'offline'}
          </span>

          {status?.git_branch && (
            <span className="source-badge online" title={`${status.git_dirty_files} dirty files`}>
              {status.git_dirty_files > 0 ? `⚡ ${status.git_branch}` : `⎇ ${status.git_branch}`}
            </span>
          )}

          {status?.uptime_secs !== undefined && (
            <span style={{ fontSize: '0.6rem', color: 'var(--text-muted)', fontFamily: "'JetBrains Mono', monospace" }}>
              ↑{formatUptime(status.uptime_secs)}
            </span>
          )}
        </div>

        {/* Source availability */}
        <div className="timeline-status-row">
          <span className={`source-badge ${daemonOnline ? 'online' : 'offline'}`}>
            terminal
          </span>
          <span className={`source-badge ${dockerOk ? 'online' : 'offline'}`}>
            docker
          </span>
          {httpCount > 0 && (
            <span className="source-badge warn">
              {http4xx > 0 && `${http4xx}×4xx`}
              {http4xx > 0 && http5xx > 0 && ' '}
              {http5xx > 0 && `${http5xx}×5xx`}
            </span>
          )}
          {httpCount === 0 && (
            <span className="source-badge offline">http proxy</span>
          )}
        </div>

        {/* Last HTTP error */}
        {httpErrors?.events?.[0] && (
          <div style={{ fontSize: '0.58rem', color: 'var(--text-muted)', fontFamily: "'JetBrains Mono', monospace", overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            last: {httpErrors.events[0].method} {httpErrors.events[0].status} — {msAgo(httpErrors.events[0].timestamp_ms)}
          </div>
        )}
      </div>
    </div>
  );
}
