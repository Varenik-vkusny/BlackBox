export type HunkLineKind = 'context' | 'added' | 'removed';

export interface HunkLine {
  kind: HunkLineKind;
  text: string;
}

export interface DiffHunk {
  file: string;
  old_start: number;
  new_start: number;
  lines: HunkLine[];
}

export interface DiffResponse {
  diff_hunks: DiffHunk[];
  files_cross_referenced: string[];
  truncated: boolean;
}

export interface BBStatus {
  uptime_secs: number;
  buffer_lines: number;
  git_branch: string | null;
  git_dirty_files: number;
  project_type: string;
  has_recent_errors: boolean;
}

export interface LogData {
  lines: string[];
}

export interface LogCluster {
  pattern: string;
  count: number;
  first_seen_ms: number;
  last_seen_ms: number;
  example: string;
  level: string | null;
}

export interface StackFrame {
  raw: string;
  file: string | null;
  line: number | null;
  is_user_code: boolean;
}

export interface StackTrace {
  language: string;
  error_message: string;
  frames: StackFrame[];
  source_files: string[];
  captured_at_ms: number;
}

export interface CompressedResponse {
  clusters: LogCluster[];
  stack_traces: StackTrace[];
  total_error_lines: number;
  fallback_source?: string;
}

export interface DockerEvent {
  source: { type: string; container_id: string };
  text: string;
  timestamp_ms: number;
  level: string | null;
}

export interface DockerResponse {
  containers: string[];
  events: DockerEvent[];
  docker_available: boolean;
}

export interface PostmortemBucket {
  minute_offset: number;
  line_count: number;
  error_count: number;
  sample: string;
}

export interface PostmortemResponse {
  window_minutes: number;
  total_lines: number;
  timeline: PostmortemBucket[];
  docker_events_in_window: number;
  stack_traces: StackTrace[];
  fallback_source: string;
}

export interface CorrelatedEvent {
  source: string;
  text: string;
  level: string | null;
}

export interface Correlation {
  terminal_line: string;
  timestamp_ms: number;
  correlated_docker_events: CorrelatedEvent[];
}

export interface CorrelatedResponse {
  correlations: Correlation[];
  has_cross_source_correlations: boolean;
  window_secs: number;
  fallback_source: string;
}
