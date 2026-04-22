import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import type {
  BBStatus,
  LogData,
  LogLine,
  DiffResponse,
  CompressedResponse,
  DockerResponse,
  PostmortemResponse,
  CorrelatedResponse,
  HttpErrorsResponse,
  WatchedFilesResponse,
  RecentCommitsResponse,
  StructuredResponse,
  StreamEntry,
} from '../types';

const API_BASE = 'http://127.0.0.1:8768/api';

function unwrapMcp(data: unknown): unknown {
  const d = data as Record<string, unknown>;
  const content = d?.content as Array<{ text?: string }> | undefined;
  if (content?.[0]?.text) {
    try { return JSON.parse(content[0].text as string); } catch { return data; }
  }
  return data;
}

export function useDaemon() {
  const [status, setStatus] = useState<BBStatus | null>(null);
  const [logLines, setLogLines] = useState<LogLine[]>([]);
  const [compressed, setCompressed] = useState<CompressedResponse | null>(null);
  const [docker, setDocker] = useState<DockerResponse | null>(null);
  const [diff, setDiff] = useState<DiffResponse | null>(null);
  const [postmortem, setPostmortem] = useState<PostmortemResponse | null>(null);
  const [correlated, setCorrelated] = useState<CorrelatedResponse | null>(null);
  const [httpErrors, setHttpErrors] = useState<HttpErrorsResponse | null>(null);
  const [watched, setWatched] = useState<WatchedFilesResponse | null>(null);
  const [commits, setCommits] = useState<RecentCommitsResponse | null>(null);
  const [structured, setStructured] = useState<StructuredResponse | null>(null);

  const [loading, setLoading] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [daemonOnline, setDaemonOnline] = useState(false);

  // Cross-component filter state
  const [selectedSource, setSelectedSource] = useState<string | null>(null);
  const [timeFilter, setTimeFilter] = useState<number | null>(null);
  const [correlationTarget, setCorrelationTarget] = useState<StreamEntry | null>(null);

  const isMounted = useRef(true);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/status`);
      const data = await res.json();
      if (isMounted.current) {
        setStatus(data);
        setDaemonOnline(true);
      }
    } catch {
      if (isMounted.current) setDaemonOnline(false);
    }
  }, []);

  const fetchLogs = useCallback(async (limit = 150) => {
    try {
      const res = await fetch(`${API_BASE}/terminal?limit=${limit}`);
      const data: LogData = await res.json();
      if (isMounted.current) setLogLines(data.lines);
    } catch (e) { void e; }
  }, []);

  const fetchCompressed = useCallback(async (source?: string | null) => {
    try {
      let querySource = source;
      if (source === 'terminal' || source === 'http') querySource = null;
      
      const url = querySource
        ? `${API_BASE}/compressed?source=${encodeURIComponent(querySource)}`
        : `${API_BASE}/compressed`;
      const res = await fetch(url);
      const data = await res.json();
      if (isMounted.current) setCompressed(unwrapMcp(data) as CompressedResponse);
    } catch (e) { void e; }
  }, []);

  const fetchDocker = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/docker`);
      const data = await res.json();
      if (isMounted.current) setDocker(data as DockerResponse);
    } catch (e) { void e; }
  }, []);

  const fetchPostmortem = useCallback(async (minutes = 30) => {
    try {
      const res = await fetch(`${API_BASE}/postmortem?minutes=${minutes}`);
      const data = await res.json();
      if (isMounted.current) setPostmortem(unwrapMcp(data) as PostmortemResponse);
    } catch (e) { void e; }
  }, []);

  const fetchCorrelated = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/correlated`);
      const data = await res.json();
      if (isMounted.current) setCorrelated(unwrapMcp(data) as CorrelatedResponse);
    } catch (e) { void e; }
  }, []);

  const fetchHttpErrors = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/http-errors?limit=50`);
      const data = await res.json();
      if (isMounted.current) setHttpErrors(unwrapMcp(data) as HttpErrorsResponse);
    } catch (e) { void e; }
  }, []);

  const fetchWatched = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/watched`);
      const data = await res.json();
      if (isMounted.current) setWatched(unwrapMcp(data) as WatchedFilesResponse);
    } catch (e) { void e; }
  }, []);

  const fetchCommits = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/commits?limit=20`);
      const data = await res.json();
      if (isMounted.current) setCommits(unwrapMcp(data) as RecentCommitsResponse);
    } catch (e) { void e; }
  }, []);

  const fetchStructured = useCallback(async (spanId?: string) => {
    try {
      const url = spanId
        ? `${API_BASE}/structured?span_id=${encodeURIComponent(spanId)}&limit=50`
        : `${API_BASE}/structured?limit=50`;
      const res = await fetch(url);
      const data = await res.json();
      if (isMounted.current) setStructured(unwrapMcp(data) as StructuredResponse);
    } catch (e) { void e; }
  }, []);

  const fetchDiff = useCallback(async (source?: string | null) => {
    setLoading(true);
    try {
      let querySource = source;
      if (source === 'terminal' || source === 'http') querySource = null;

      const url = querySource
        ? `${API_BASE}/diff?source=${encodeURIComponent(querySource)}`
        : `${API_BASE}/diff`;
      const res = await fetch(url);
      const data = await res.json();
      if (!isMounted.current) return;
      setDiff(unwrapMcp(data) as DiffResponse);
    } catch (e) { void e; } finally {
      if (isMounted.current) setLoading(false);
    }
  }, []);

  const injectLog = useCallback(async (text: string) => {
    try {
      await fetch(`${API_BASE}/inject`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text }),
      });
      fetchLogs();
      fetchCompressed();
    } catch (e) { void e; }
  }, [fetchLogs, fetchCompressed]);

  const clearLogs = useCallback(async () => {
    try {
      await fetch(`${API_BASE}/clear`, { method: 'POST' });
      setLogLines([]);
      setCompressed(null);
      setPostmortem(null);
      setCorrelated(null);
      setHttpErrors(null);
      setStructured(null);
      fetchStatus();
    } catch (e) { void e; }
  }, [fetchStatus]);

  useEffect(() => {
    isMounted.current = true;

    const init = async () => {
      await Promise.all([
        fetchStatus(),
        fetchLogs(),
        fetchCompressed(selectedSource),
        fetchDocker(),
        fetchPostmortem(),
        fetchCorrelated(),
        fetchHttpErrors(),
        fetchWatched(),
        fetchCommits(),
        fetchStructured(),
        fetchDiff(selectedSource),
      ]);
    };

    if (!isPaused) init();

    // Fast poll: live data sources (3s)
    const fast = setInterval(() => {
      if (!isPaused) {
        fetchStatus();
        fetchLogs();
        fetchCompressed(selectedSource);
        fetchDocker();
        fetchPostmortem();
        fetchCorrelated();
        fetchHttpErrors();
        fetchStructured();
        fetchDiff(selectedSource);
      }
    }, 3000);

    // Slow poll: git state and watched files (10s)
    const slow = setInterval(() => {
      if (!isPaused) {
        fetchWatched();
        fetchCommits();
      }
    }, 10000);

    return () => {
      isMounted.current = false;
      clearInterval(fast);
      clearInterval(slow);
    };
  }, [
    isPaused, selectedSource,
    fetchStatus, fetchLogs, fetchCompressed, fetchDocker,
    fetchPostmortem, fetchCorrelated, fetchHttpErrors,
    fetchWatched, fetchCommits, fetchStructured, fetchDiff
  ]);

  // Derive plain string[] for components that don't need session metadata
  const logs = useMemo(() => logLines.map(l => l.text), [logLines]);

  return {
    // Data
    status, logs, logLines, compressed, docker, diff, postmortem, correlated,
    httpErrors, watched, commits, structured,
    // UI state
    loading, isPaused, daemonOnline,
    // Filter state
    selectedSource, setSelectedSource,
    timeFilter, setTimeFilter,
    correlationTarget, setCorrelationTarget,
    // Actions
    setIsPaused,
    refreshDiff: fetchDiff,
    refreshStructured: fetchStructured,
    injectLog,
    clearLogs,
  };
}
