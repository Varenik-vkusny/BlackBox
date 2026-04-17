import * as net from 'net';
import * as vscode from 'vscode';

// onDidWriteTerminalData is a proposed VS Code API not yet in stable @types/vscode.
// We declare the minimal types here so the extension compiles without enableProposedApi.
interface TerminalDataWriteEvent {
    readonly terminal: vscode.Terminal;
    readonly data: string;
}
interface WindowWithProposedApi {
    onDidWriteTerminalData: vscode.Event<TerminalDataWriteEvent>;
}
const windowEx = vscode.window as unknown as WindowWithProposedApi;

const DAEMON_PORT = 8765;
const DAEMON_HOST = '127.0.0.1';
const RECONNECT_DELAY_MS = 3000;

let socket: net.Socket | null = null;
let reconnectTimer: NodeJS.Timeout | null = null;
let isDeactivating = false;

export function activate(context: vscode.ExtensionContext): void {
    connect();

    const listener = windowEx.onDidWriteTerminalData((event: TerminalDataWriteEvent) => {
        if (socket && !socket.destroyed) {
            // Send raw data — daemon handles ANSI stripping
            socket.write(event.data);
            // Ensure newline termination for line-based reading
            if (!event.data.endsWith('\n')) {
                socket.write('\n');
            }
        }
    });

    context.subscriptions.push(listener);
}

export function deactivate(): void {
    isDeactivating = true;
    if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
    }
    if (socket) {
        socket.destroy();
        socket = null;
    }
}

function connect(): void {
    if (isDeactivating) return;

    socket = net.createConnection({ port: DAEMON_PORT, host: DAEMON_HOST });

    socket.on('connect', () => {
        // Connection established — no user notification, plugin is invisible
    });

    socket.on('error', () => {
        // Daemon not running — schedule reconnect silently
        scheduleReconnect();
    });

    socket.on('close', () => {
        if (!isDeactivating) {
            scheduleReconnect();
        }
    });
}

function scheduleReconnect(): void {
    if (isDeactivating || reconnectTimer) return;
    socket = null;
    reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        connect();
    }, RECONNECT_DELAY_MS);
}
