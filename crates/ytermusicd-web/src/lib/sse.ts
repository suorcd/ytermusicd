import type { Event as ApiEvent } from '@api/Event';
import type { EventKind } from '@api/EventKind';

export type ConnectionState = 'connecting' | 'online' | 'reconnecting' | 'offline';

export interface EventSourceLike {
  onopen: ((event: Event) => void) | null;
  onerror: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  addEventListener(type: string, listener: (event: MessageEvent) => void): void;
  close(): void;
}

export interface EventStreamOptions {
  url?: string;
  create?: (url: string) => EventSourceLike;
  onEvent: (event: EventKind) => void;
  onState: (state: ConnectionState) => void;
  onParseError?: (error: unknown) => void;
  offlineAfterMs?: number;
  retryBaseMs?: number;
  retryMaxMs?: number;
  setTimeoutFn?: typeof setTimeout;
  clearTimeoutFn?: typeof clearTimeout;
}

interface ResolvedOptions {
  url: string;
  create: (url: string) => EventSourceLike;
  onEvent: (event: EventKind) => void;
  onState: (state: ConnectionState) => void;
  onParseError?: (error: unknown) => void;
  offlineAfterMs: number;
  retryBaseMs: number;
  retryMaxMs: number;
  setTimeoutFn: typeof setTimeout;
  clearTimeoutFn: typeof clearTimeout;
}

function defaultCreate(url: string): EventSourceLike {
  return new EventSource(url) as unknown as EventSourceLike;
}

/**
 * EventSource wrapper with explicit exponential backoff. The last event id is
 * sent as a query parameter, because a new EventSource cannot set headers.
 * The daemon accepts both the query parameter and the standard header.
 */
export class EventStream {
  private readonly options: ResolvedOptions;
  private source: EventSourceLike | null = null;
  private offlineTimer: ReturnType<typeof setTimeout> | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private attempts = 0;
  private lastEventId: number | null = null;
  private closed = false;

  constructor(options: EventStreamOptions) {
    this.options = {
      url: '/api/events',
      create: defaultCreate,
      offlineAfterMs: 5000,
      retryBaseMs: 1000,
      retryMaxMs: 30000,
      setTimeoutFn: setTimeout,
      clearTimeoutFn: clearTimeout,
      ...options,
    };
  }

  connect(): void {
    this.closed = false;
    this.attempts = 0;
    this.open(false);
  }

  retry(): void {
    this.connect();
  }

  close(): void {
    this.closed = true;
    this.clearOfflineTimer();
    this.clearRetryTimer();
    this.disposeSource();
  }

  private open(reconnecting: boolean): void {
    this.disposeSource();
    this.options.onState(reconnecting ? 'reconnecting' : 'connecting');

    const url =
      this.lastEventId === null
        ? this.options.url
        : `${this.options.url}?last_event_id=${this.lastEventId}`;
    const source = this.options.create(url);
    this.source = source;

    source.onopen = () => {
      this.attempts = 0;
      this.clearOfflineTimer();
      this.clearRetryTimer();
      this.options.onState('online');
    };

    source.onerror = () => {
      if (this.closed) return;
      this.options.onState('reconnecting');
      this.disposeSource();
      this.startOfflineTimer();
      this.scheduleReconnect();
    };

    source.addEventListener('snapshot', (event) => this.handleMessage(event));
    source.onmessage = (event) => this.handleMessage(event);
  }

  private scheduleReconnect(): void {
    this.clearRetryTimer();
    const delay = Math.min(
      this.options.retryBaseMs * 2 ** this.attempts,
      this.options.retryMaxMs,
    );
    this.attempts += 1;
    this.retryTimer = this.options.setTimeoutFn(() => {
      this.retryTimer = null;
      if (!this.closed) this.open(true);
    }, delay);
  }

  private startOfflineTimer(): void {
    if (this.offlineTimer !== null) return;
    this.offlineTimer = this.options.setTimeoutFn(() => {
      this.offlineTimer = null;
      if (!this.closed) this.options.onState('offline');
    }, this.options.offlineAfterMs);
  }

  private disposeSource(): void {
    if (this.source) {
      this.source.onopen = null;
      this.source.onerror = null;
      this.source.onmessage = null;
      this.source.close();
      this.source = null;
    }
  }

  private clearOfflineTimer(): void {
    if (this.offlineTimer !== null) {
      this.options.clearTimeoutFn(this.offlineTimer);
      this.offlineTimer = null;
    }
  }

  private clearRetryTimer(): void {
    if (this.retryTimer !== null) {
      this.options.clearTimeoutFn(this.retryTimer);
      this.retryTimer = null;
    }
  }

  private handleMessage(message: MessageEvent): void {
    try {
      const envelope = JSON.parse(String(message.data)) as ApiEvent;
      if (typeof envelope?.id === 'number') this.lastEventId = envelope.id;
      if (envelope?.event) this.options.onEvent(envelope.event);
    } catch (error) {
      this.options.onParseError?.(error);
    }
  }
}
