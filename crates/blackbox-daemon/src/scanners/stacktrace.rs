use std::time::{SystemTime, UNIX_EPOCH};

use blackbox_core::types::{LogLine, ParsedStackTrace, StackFrame};

/// Parse multi-language stack traces from a slice of terminal log lines.
pub fn extract_stack_traces(lines: &[LogLine]) -> Vec<ParsedStackTrace> {
    let mut results = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let text = &lines[i].text;

        if let Some((trace, next_i)) = try_parse_rust(lines, i) {
            i = next_i;
            results.push(trace);
        } else if let Some((trace, next_i)) = try_parse_python(lines, i) {
            i = next_i;
            results.push(trace);
        } else if let Some((trace, next_i)) = try_parse_nodejs(lines, i) {
            i = next_i;
            results.push(trace);
        } else if let Some((trace, next_i)) = try_parse_javac(lines, i) {
            i = next_i;
            results.push(trace);
        } else if let Some((trace, next_i)) = try_parse_java(lines, i) {
            i = next_i;
            results.push(trace);
        } else if let Some((trace, next_i)) = try_parse_go(lines, i) {
            i = next_i;
            results.push(trace);
        } else {
            let _ = text; // suppress unused warning
            i += 1;
        }
    }

    results
}

/// Collect deduplicated source file paths from user-code frames.
pub fn extract_source_files(traces: &[ParsedStackTrace]) -> Vec<String> {
    let mut files: Vec<String> = traces
        .iter()
        .flat_map(|t| t.source_files.iter().cloned())
        .collect();
    files.sort();
    files.dedup();
    files
}

// ── Rust panic ────────────────────────────────────────────────────────────────
// Trigger: "thread '...' panicked at"
// Frames:  lines starting with spaces containing "::" (symbol names)
// File refs: lines matching "   --> src/..."

fn try_parse_rust(lines: &[LogLine], start: usize) -> Option<(ParsedStackTrace, usize)> {
    let trigger = &lines[start].text;
    let is_panic = trigger.contains("panicked at");
    let is_compiler_error = trigger.trim_start().starts_with("error[") || trigger.trim_start().starts_with("error: ");
    if !is_panic && !is_compiler_error {
        return None;
    }

    let mut frames = Vec::new();
    let mut source_files = Vec::new();
    let mut j = start + 1;

    while j < lines.len() {
        let text = &lines[j].text;
        let trimmed = text.trim();

        // Runtime backtrace frame: "   N: some::module::function"
        if trimmed.starts_with(|c: char| c.is_ascii_digit()) && trimmed.contains("::") {
            let is_user = !is_rust_stdlib_frame(trimmed);
            frames.push(StackFrame {
                raw: trimmed.to_string(),
                file: None,
                line: None,
                is_user_code: is_user,
            });
        } else if trimmed.starts_with("at ") {
            // "at src/main.rs:42:10" — location for the preceding frame
            let path_part = trimmed.trim_start_matches("at ").split(':').next().unwrap_or("");
            if !path_part.is_empty() && is_user_file(path_part) {
                if let Some(last) = frames.last_mut() {
                    last.file = Some(path_part.to_string());
                    last.line = trimmed.split(':').nth(1).and_then(|s| s.parse().ok());
                }
                source_files.push(path_part.to_string());
            }
        } else if trimmed.starts_with("-->") {
            // Compiler error location: "  --> src/main.rs:10:5"
            let rest = trimmed.trim_start_matches("-->").trim();
            let mut parts = rest.splitn(3, ':');
            let path_part = parts.next().unwrap_or("");
            let line_no: Option<u32> = parts.next().and_then(|s| s.parse().ok());

            if is_user_file(path_part) {
                source_files.push(path_part.to_string());
                frames.push(StackFrame {
                    raw: format!("error at {}:{}", path_part, line_no.unwrap_or(0)),
                    file: Some(path_part.to_string()),
                    line: line_no,
                    is_user_code: true,
                });
            }
        } else if trimmed.starts_with('|') || trimmed.starts_with('=') || trimmed.starts_with("note:") || trimmed.starts_with("help:") {
            // Compiler diagnostic context lines — skip but keep scanning.
        } else if !frames.is_empty() && trimmed.starts_with("error[") {
            // Another compiler error block starts — stop here.
            break;
        } else if (frames.len() >= 3 && !trimmed.starts_with(|c: char| c.is_ascii_digit() || c == 'a' || c == '|' || c == '=')) || j - start > 40 {
            break;
        }
        j += 1;
    }

    let min_frames = if is_compiler_error { 1 } else { 2 };
    if frames.len() < min_frames {
        return None;
    }

    source_files.sort();
    source_files.dedup();

    Some((ParsedStackTrace {
        language: "rust".into(),
        error_message: trigger.trim().to_string(),
        frames,
        source_files,
        captured_at_ms: now_ms(),
    }, j))
}

fn is_rust_stdlib_frame(frame: &str) -> bool {
    let stdlib = ["std::", "core::", "tokio::", "alloc::", "rustc_", "<std::", "<core::"];
    stdlib.iter().any(|p| frame.contains(p))
}

// ── Python traceback ──────────────────────────────────────────────────────────
// Trigger: "Traceback (most recent call last):"
// Frames:  '  File "..." line N'
// End:     non-indented line (the exception message)

fn try_parse_python(lines: &[LogLine], start: usize) -> Option<(ParsedStackTrace, usize)> {
    if !lines[start].text.contains("Traceback (most recent call last)") {
        return None;
    }

    let mut frames = Vec::new();
    let mut source_files = Vec::new();
    let mut error_msg = String::new();
    let mut j = start + 1;

    while j < lines.len() {
        let raw = &lines[j].text;
        let trimmed = raw.trim();
        let is_indented = raw.starts_with(' ') || raw.starts_with('\t');

        if trimmed.starts_with("File \"") {
            // 'File "/path/to/file.py", line 42, in func_name'
            let is_user = !trimmed.contains("site-packages");
            let file = extract_between(trimmed, "File \"", "\"");
            let lineno = extract_between(trimmed, "line ", ",")
                .or_else(|| trimmed.split("line ").nth(1).map(|s| s.trim().to_string()))
                .and_then(|s| s.parse().ok());
            if let Some(ref f) = file {
                if is_user {
                    source_files.push(f.clone());
                }
            }
            frames.push(StackFrame {
                raw: trimmed.to_string(),
                file,
                line: lineno,
                is_user_code: is_user,
            });
        } else if !is_indented && !trimmed.is_empty() && j > start + 1 {
            // Non-indented line after frames = exception message.
            error_msg = trimmed.to_string();
            j += 1; // advance past the error message line
            break;
        } else if j - start > 50 {
            break;
        }
        j += 1;
    }

    if frames.is_empty() {
        return None;
    }

    source_files.sort();
    source_files.dedup();

    Some((ParsedStackTrace {
        language: "python".into(),
        error_message: error_msg,
        frames,
        source_files,
        captured_at_ms: now_ms(),
    }, j))
}

// ── Node.js / TypeScript ──────────────────────────────────────────────────────
// Trigger: "Error:" / "TypeError:" / "ReferenceError:" etc.
// Frames:  lines starting with "    at "

fn try_parse_nodejs(lines: &[LogLine], start: usize) -> Option<(ParsedStackTrace, usize)> {
    let trigger = &lines[start].text;
    let is_error_line = trigger.trim_start().starts_with("Error:")
        || is_nodejs_error_type(trigger.trim_start());
    if !is_error_line {
        return None;
    }

    let mut frames = Vec::new();
    let mut source_files = Vec::new();
    let mut j = start + 1;

    while j < lines.len() {
        let text = &lines[j].text;
        let trimmed = text.trim();
        if trimmed.starts_with("at ") {
            let is_user = !is_node_stdlib_frame(trimmed);
            let (file, line) = parse_node_frame_location(trimmed);
            if is_user {
                if let Some(ref f) = file {
                    source_files.push(f.clone());
                }
            }
            frames.push(StackFrame {
                raw: trimmed.to_string(),
                file,
                line,
                is_user_code: is_user,
            });
        } else if !frames.is_empty() || j - start > 3 {
            break;
        }
        j += 1;
    }

    if frames.is_empty() {
        return None;
    }

    source_files.sort();
    source_files.dedup();

    Some((ParsedStackTrace {
        language: "nodejs".into(),
        error_message: trigger.trim().to_string(),
        frames,
        source_files,
        captured_at_ms: now_ms(),
    }, j))
}

fn is_nodejs_error_type(s: &str) -> bool {
    // Exhaustive list of built-in JS/TS error constructors.
    // Intentionally excludes "ConnectionError", "RuntimeError", etc. (Python/Java).
    let types = [
        "TypeError:", "ReferenceError:", "SyntaxError:", "RangeError:",
        "URIError:", "EvalError:", "InternalError:", "AggregateError:",
    ];
    types.iter().any(|t| s.starts_with(t))
}

fn is_node_stdlib_frame(frame: &str) -> bool {
    frame.contains("node_modules") || frame.contains("node:internal") || frame.contains("(internal/")
}

fn parse_node_frame_location(frame: &str) -> (Option<String>, Option<u32>) {
    // "at Function.foo (/path/to/file.js:42:10)"
    if let Some(start) = frame.rfind('(') {
        let inner = &frame[start + 1..frame.rfind(')').unwrap_or(frame.len())];
        let parts: Vec<&str> = inner.rsplitn(3, ':').collect();
        if parts.len() >= 2 {
            let file = parts.last().map(|s| s.to_string());
            let line = parts.get(1).and_then(|s| s.parse().ok());
            return (file, line);
        }
    }
    (None, None)
}

// ── Java / JVM ────────────────────────────────────────────────────────────────
// Trigger: "Exception in thread" / "Caused by:" / "java.lang.Exception:"
// Frames:  lines starting with "\tat " (note: tab is stripped by trim())

// ── javac compiler error ──────────────────────────────────────────────────────
// Format:  "Filename.java:LINE: error: MESSAGE"
// No runtime frames — synthesize one from the filename:line in the trigger.

fn try_parse_javac(lines: &[LogLine], start: usize) -> Option<(ParsedStackTrace, usize)> {
    let trigger = &lines[start].text;
    let trimmed = trigger.trim();

    if !trimmed.contains(".java:") {
        return None;
    }
    let dot_java_pos = trimmed.find(".java:").unwrap();
    let after_java = &trimmed[dot_java_pos + 5..];
    let mut rest = after_java.trim_start_matches(':');
    let colon_pos = rest.find(':').unwrap_or(rest.len());
    let line_no: Option<u32> = rest[..colon_pos].trim().parse().ok();
    rest = &rest[colon_pos..];
    let rest = rest.trim_start_matches(':').trim();

    if !rest.starts_with("error:") && !rest.starts_with("warning:") {
        return None;
    }

    let file_name = &trimmed[..dot_java_pos + 5];
    let file_name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);

    let source_files = if is_user_file(file_name) { vec![file_name.to_string()] } else { vec![] };
    let frame = StackFrame {
        raw: trimmed.to_string(),
        file: if is_user_file(file_name) { Some(file_name.to_string()) } else { None },
        line: line_no,
        is_user_code: is_user_file(file_name),
    };

    // Advance past the error block: skip context lines (source text, carets, symbol info)
    // until we see a line that looks like the start of something new or a blank summary.
    let mut j = start + 1;
    while j < lines.len() {
        let t = lines[j].text.trim();
        // "N error" / "N warning" summary line signals end of this error block.
        if t.ends_with("error") || t.ends_with("errors") || t.ends_with("warning") || t.ends_with("warnings") {
            j += 1; // include the summary line
            break;
        }
        // Another .java:N: error/warning — next error block starts.
        if t.contains(".java:") {
            break;
        }
        // Hard cap: don't consume more than 10 context lines.
        if j - start > 10 {
            break;
        }
        j += 1;
    }

    Some((ParsedStackTrace {
        language: "java".into(),
        error_message: trimmed.to_string(),
        frames: vec![frame],
        source_files,
        captured_at_ms: now_ms(),
    }, j))
}

fn try_parse_java(lines: &[LogLine], start: usize) -> Option<(ParsedStackTrace, usize)> {
    let trigger = &lines[start].text;
    let trimmed_trigger = trigger.trim();
    let is_java = trimmed_trigger.contains("Exception in thread")
        || trimmed_trigger.contains("Caused by:")
        || (trimmed_trigger.contains("Exception:") && trimmed_trigger.contains("java."))
        || trimmed_trigger.starts_with("java.")
        || trimmed_trigger.starts_with("javax.")
        || (trimmed_trigger.ends_with("Exception:") || trimmed_trigger.contains("Exception: "))
            && !trimmed_trigger.starts_with("Error:")
            && !trimmed_trigger.starts_with("TypeError:");
    if !is_java {
        return None;
    }

    let mut frames = Vec::new();
    let mut source_files = Vec::new();
    let mut j = start + 1;

    while j < lines.len() {
        let text = &lines[j].text;
        let trimmed = text.trim();
        if trimmed.starts_with("at ") {
            let is_user = !is_java_stdlib_frame(trimmed);
            let (file, line) = parse_java_frame_location(trimmed);
            if is_user {
                if let Some(ref f) = file {
                    source_files.push(f.clone());
                }
            }
            frames.push(StackFrame {
                raw: trimmed.to_string(),
                file,
                line,
                is_user_code: is_user,
            });
        } else if !frames.is_empty() {
            if trimmed.starts_with("Caused by:") || trimmed.starts_with("...") {
                // Continue: chained exception or truncated frame marker
            } else {
                break;
            }
        } else if j - start > 5 {
            break;
        }
        j += 1;
    }

    if frames.is_empty() {
        return None;
    }

    source_files.sort();
    source_files.dedup();

    Some((ParsedStackTrace {
        language: "java".into(),
        error_message: trigger.trim().to_string(),
        frames,
        source_files,
        captured_at_ms: now_ms(),
    }, j))
}

fn is_java_stdlib_frame(frame: &str) -> bool {
    let stdlib = ["java.", "javax.", "sun.", "com.sun.", "jdk.", "org.junit.", "org.springframework."];
    stdlib.iter().any(|p| frame.contains(p))
}

fn parse_java_frame_location(frame: &str) -> (Option<String>, Option<u32>) {
    // "at com.example.MyClass.method(MyClass.java:42)"
    if let (Some(start), Some(end)) = (frame.rfind('('), frame.rfind(')')) {
        let inner = &frame[start + 1..end];
        let parts: Vec<&str> = inner.split(':').collect();
        let file = parts.first().map(|s| s.to_string());
        let line = parts.get(1).and_then(|s| s.parse().ok());
        return (file, line);
    }
    (None, None)
}

// ── Go panic / goroutine dump ─────────────────────────────────────────────────
// Trigger: "panic: ..." or "goroutine N [running]:"
// Frames come in PAIRS: function name line (unindented) + file path (tab-indented)
// Example:
//   panic: runtime error: index out of range [0] with length 0
//   goroutine 1 [running]:
//   main.handler(0xc0000b4000, 0xc0000b2000)
//           /app/main.go:42 +0x80
//   main.main()
//           /app/main.go:15 +0x2c

fn try_parse_go(lines: &[LogLine], start: usize) -> Option<(ParsedStackTrace, usize)> {
    let trigger = &lines[start].text;
    let trimmed = trigger.trim();

    let is_panic = trimmed.starts_with("panic:");
    let is_goroutine = trimmed.starts_with("goroutine ") && trimmed.contains('[') && trimmed.ends_with(':');
    if !is_panic && !is_goroutine {
        return None;
    }

    let error_message = trimmed.to_string();
    let mut frames: Vec<StackFrame> = Vec::new();
    let mut source_files = Vec::new();
    let mut j = start + 1;
    let mut pending_func: Option<String> = None;

    while j < lines.len() {
        let raw = &lines[j].text;
        let trimmed_j = raw.trim();

        if trimmed_j.is_empty() {
            // Empty line after frames signals end of goroutine section
            if !frames.is_empty() { break; }
            j += 1;
            continue;
        }

        if trimmed_j == "exit status 2" || trimmed_j == "exit status 1" {
            break;
        }

        // Tab-indented line = file:line for the previous function
        if raw.starts_with('\t') || raw.starts_with("        ") {
            if let Some(func) = pending_func.take() {
                let path_part = trimmed_j.split(' ').next().unwrap_or(trimmed_j);
                let (file, line) = parse_go_file_location(path_part);
                let is_user = !is_go_stdlib_frame(&func);
                if is_user {
                    if let Some(ref f) = file {
                        source_files.push(f.clone());
                    }
                }
                frames.push(StackFrame {
                    raw: format!("{func}\n\t{trimmed_j}"),
                    file,
                    line,
                    is_user_code: is_user,
                });
            }
            j += 1;
            continue;
        }

        // Goroutine header — skip if we haven't started collecting frames yet
        if trimmed_j.starts_with("goroutine ") && trimmed_j.contains('[') {
            if !frames.is_empty() { break; } // new goroutine = stop current trace
            j += 1;
            continue;
        }

        // Non-indented, non-goroutine line = function name
        pending_func = Some(trimmed_j.to_string());

        if j - start > 80 { break; }
        j += 1;
    }

    if frames.is_empty() {
        return None;
    }

    source_files.sort();
    source_files.dedup();

    Some((ParsedStackTrace {
        language: "go".into(),
        error_message,
        frames,
        source_files,
        captured_at_ms: now_ms(),
    }, j))
}

fn parse_go_file_location(path: &str) -> (Option<String>, Option<u32>) {
    // "/app/main.go:42" or "/app/main.go:42 +0x80"
    let path = path.split(' ').next().unwrap_or(path);
    // Use rsplitn to handle Windows-style paths like C:\app\main.go:42
    let parts: Vec<&str> = path.rsplitn(2, ':').collect();
    if parts.len() == 2 {
        let line: Option<u32> = parts[0].parse().ok();
        let file = parts[1].to_string();
        return (Some(file), line);
    }
    (Some(path.to_string()), None)
}

fn is_go_stdlib_frame(func: &str) -> bool {
    func.starts_with("runtime.")
        || func.starts_with("reflect.")
        || func.starts_with("testing.")
        || func.contains("runtime/")
        || func.starts_with("sync.")
        || func.starts_with("net/http.")
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn is_user_file(path: &str) -> bool {
    !path.contains("/.rustup/")
        && !path.contains("/registry/src/")
        && !path.contains("site-packages")
        && !path.contains("node_modules")
        && !path.is_empty()
}

fn extract_between(s: &str, after: &str, before: &str) -> Option<String> {
    let start = s.find(after)? + after.len();
    let end = s[start..].find(before)? + start;
    Some(s[start..end].to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(texts: &[&str]) -> Vec<LogLine> {
        texts
            .iter()
            .map(|t| LogLine { text: t.to_string(), timestamp_ms: 0, source_terminal: None })
            .collect()
    }

    #[test]
    fn parses_rust_panic() {
        let input = lines(&[
            "thread 'main' panicked at 'index out of bounds', src/main.rs:10:5",
            "   0: myapp::do_thing",
            "      at src/main.rs:10",
            "   1: myapp::main",
            "      at src/main.rs:20",
            "   2: std::rt::lang_start",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse rust panic");
        assert_eq!(traces[0].language, "rust");
        assert!(traces[0].frames.iter().any(|f| f.is_user_code));
        assert!(traces[0].frames.iter().any(|f| !f.is_user_code)); // std frame filtered
    }

    #[test]
    fn parses_python_traceback() {
        let input = lines(&[
            "Traceback (most recent call last):",
            "  File \"src/app.py\", line 42, in run",
            "    result = process(data)",
            "  File \"src/utils.py\", line 10, in process",
            "    return data[key]",
            "KeyError: 'missing_field'",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse python traceback");
        assert_eq!(traces[0].language, "python");
        assert!(traces[0].source_files.contains(&"src/app.py".to_string()));
        assert_eq!(traces[0].error_message, "KeyError: 'missing_field'");
    }

    #[test]
    fn parses_nodejs_error() {
        let input = lines(&[
            "TypeError: Cannot read properties of undefined (reading 'name')",
            "    at getUserName (/app/src/users.js:15:20)",
            "    at handler (/app/src/routes.js:42:10)",
            "    at Layer.handle (node_modules/express/lib/router/layer.js:95:5)",
            "    at next (node:internal/module.js:45:3)",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse nodejs error");
        assert_eq!(traces[0].language, "nodejs");
        // stdlib frames should be filtered
        assert!(traces[0].frames.iter().filter(|f| f.is_user_code).count() >= 2);
    }

    #[test]
    fn parses_java_exception() {
        let input = lines(&[
            "Exception in thread \"main\" java.lang.NullPointerException: Cannot invoke method",
            "    at com.example.MyService.process(MyService.java:55)",
            "    at com.example.Main.run(Main.java:20)",
            "    at java.lang.Thread.run(Thread.java:748)",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse java exception");
        assert_eq!(traces[0].language, "java");
        assert!(traces[0].frames.iter().any(|f| f.is_user_code));
    }

    #[test]
    fn parses_java_exception_bare_prefix() {
        // Injection Station preset format: starts with "java.lang.*Exception:"
        // without the "Exception in thread" prefix.
        let input = lines(&[
            "java.lang.NullPointerException: Cannot invoke method handle()",
            "\tat com.example.api.Controller.process(Controller.java:77)",
            "\tat com.example.api.Router.dispatch(Router.java:31)",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse bare java.lang.* exception");
        assert_eq!(traces[0].language, "java");
        assert!(traces[0].frames.iter().any(|f| f.is_user_code));
        assert!(traces[0].source_files.iter().any(|f| f.contains("Controller.java")));
    }

    #[test]
    fn parses_rust_compiler_error() {
        // rustc / rust-script compiler error format — no runtime backtrace, only --> location.
        let input = lines(&[
            "error[E0425]: cannot find value `ngfbjkdvlsdfsgfdgdfg` in this scope",
            " --> main.rs:3:6",
            "  |",
            "3 |     {ngfbjkdvlsdfsgfdgdfg}",
            "  |      ^^^^^^^^^^^^^^^^^^^^",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse rust compiler error");
        assert_eq!(traces[0].language, "rust");
        // Synthetic frame must reference the source file.
        assert!(traces[0].source_files.iter().any(|f| f == "main.rs"));
        assert!(traces[0].frames.iter().any(|f| f.file.as_deref() == Some("main.rs")));
    }

    #[test]
    fn parses_javac_compiler_error() {
        // javac output: "FileName.java:LINE: error: MESSAGE"
        let input = lines(&[
            "Main.java:2: error: cannot find symbol",
            "    SDKFJKLDSJFKLSDF;",
            "    ^",
            "  symbol:   variable SDKFJKLDSJFKLSDF",
            "  location: class Main",
            "1 error",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse javac compiler error");
        assert_eq!(traces[0].language, "java");
        assert!(traces[0].source_files.iter().any(|f| f == "Main.java"));
        assert!(traces[0].frames.iter().any(|f| f.file.as_deref() == Some("Main.java")));
        assert_eq!(traces[0].frames[0].line, Some(2));
    }

    #[test]
    fn parses_java_single_frame_runtime() {
        // JVM error with only 1 user frame (e.g. unresolved compilation problem at runtime)
        let input = lines(&[
            "Exception in thread \"main\" java.lang.Error: Unresolved compilation problem:",
            "\tSDKFJKLDSJFKLSDF cannot be resolved to a variable",
            "\tat Main.main(Main.java:2)",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse single-frame java exception");
        assert_eq!(traces[0].language, "java");
        assert!(traces[0].source_files.iter().any(|f| f == "Main.java"));
    }

    #[test]
    fn parses_go_panic() {
        let input = lines(&[
            "panic: runtime error: index out of range [0] with length 0",
            "",
            "goroutine 1 [running]:",
            "main.handler(0xc0000b4000)",
            "\t/app/main.go:42 +0x80",
            "main.main()",
            "\t/app/main.go:15 +0x2c",
            "exit status 2",
        ]);
        let traces = extract_stack_traces(&input);
        assert!(!traces.is_empty(), "should parse go panic");
        assert_eq!(traces[0].language, "go");
        assert!(traces[0].source_files.iter().any(|f| f.contains("main.go")));
        assert_eq!(traces[0].frames.len(), 2);
    }

    #[test]
    fn extract_source_files_deduplicates() {
        let traces = vec![
            ParsedStackTrace {
                language: "rust".into(),
                error_message: "err".into(),
                frames: vec![],
                source_files: vec!["src/main.rs".into(), "src/lib.rs".into()],
                captured_at_ms: 0,
            },
            ParsedStackTrace {
                language: "rust".into(),
                error_message: "err2".into(),
                frames: vec![],
                source_files: vec!["src/main.rs".into()],
                captured_at_ms: 0,
            },
        ];
        let files = extract_source_files(&traces);
        assert_eq!(files, vec!["src/lib.rs", "src/main.rs"]);
    }
}
