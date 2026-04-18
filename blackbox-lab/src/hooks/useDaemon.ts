import { useState, useEffect, useCallback, useRef } from 'react';
import type {
  BBStatus,
  LogData,
  DiffResponse,
  CompressedResponse,
  DockerResponse,
  PostmortemResponse,
  CorrelatedResponse,
} from '../types';

const API_BASE = 'http://127.0.0.1:8768/api';

export function useDaemon() {
  const [status, setStatus] = useState<BBStatus | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [compressed, setCompressed] = useState<CompressedResponse | null>(null);
  const [docker, setDocker] = useState<DockerResponse | null>(null);
  const [diff, setDiff] = useState<DiffResponse | null>(null);
  const [postmortem, setPostmortem] = useState<PostmortemResponse | null>(null);
  const [correlated, setCorrelated] = useState<CorrelatedResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [daemonOnline, setDaemonOnline] = useState(false);
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
      if (isMounted.current) setLogs(data.lines);
    } catch (e) { void e; }
  }, []);

  const fetchCompressed = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/compressed`);
      const data = await res.json();
      if (isMounted.current) setCompressed(data.result || data);
    } catch (e) { void e; }
  }, []);

  const fetchDocker = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/docker`);
      const data = await res.json();
      if (isMounted.current) setDocker(data);
    } catch (e) { void e; }
  }, []);

  const fetchPostmortem = useCallback(async (minutes = 30) => {
    try {
      const res = await fetch(`${API_BASE}/postmortem?limit=${minutes}`);
      const data = await res.json();
      if (isMounted.current) setPostmortem(data.result || data);
    } catch (e) { void e; }
  }, []);

  const fetchCorrelated = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/correlated`);
      const data = await res.json();
      if (isMounted.current) setCorrelated(data.result || data);
    } catch (e) { void e; }
  }, []);

  const fetchDiff = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(`${API_BASE}/diff`);
      const data = await res.json();
      if (!isMounted.current) return;
      setDiff(data.result || data);
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
      setLogs([]);
      setCompressed(null);
      setPostmortem(null);
      setCorrelated(null);
      fetchStatus();
    } catch (e) { void e; }
  }, [fetchStatus]);

  useEffect(() => {
    isMounted.current = true;

    const init = async () => {
      await Promise.all([
        fetchStatus(),
        fetchLogs(),
        fetchCompressed(),
        fetchDocker(),
        fetchPostmortem(),
        fetchCorrelated(),
      ]);
    };

    if (!isPaused) init();

    const interval = setInterval(() => {
      if (!isPaused) {
        fetchStatus();
        fetchLogs();
        fetchCompressed();
        fetchDocker();
        fetchPostmortem();
        fetchCorrelated();
      }
    }, 3000);

    return () => {
      isMounted.current = false;
      clearInterval(interval);
    };
  }, [isPaused, fetchStatus, fetchLogs, fetchCompressed, fetchDocker, fetchPostmortem, fetchCorrelated]);

  return {
    status,
    logs,
    compressed,
    docker,
    diff,
    postmortem,
    correlated,
    loading,
    isPaused,
    daemonOnline,
    setIsPaused,
    refreshDiff: fetchDiff,
    injectLog,
    clearLogs,
  };
}
