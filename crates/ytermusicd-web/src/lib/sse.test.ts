import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EventStream } from './sse';
import { FakeEventSource } from '../test/fixtures';

describe('EventStream', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('connects, reports state and dispatches named and unnamed events', () => {
    const sources: FakeEventSource[] = [];
    const events: unknown[] = [];
    const states: string[] = [];
    const stream = new EventStream({
      create: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
      onEvent: (event) => events.push(event),
      onState: (state) => states.push(state),
    });

    stream.connect();
    expect(states).toEqual(['connecting']);
    expect(sources[0].url).toBe('/api/events');

    sources[0].open();
    expect(states).toEqual(['connecting', 'online']);

    sources[0].emitNamed('snapshot', { type: 'library_changed' });
    sources[0].emit({ type: 'track_ended', video_id: 'v1' });
    expect(events).toHaveLength(2);

    stream.close();
    expect(sources[0].closed).toBe(true);
  });

  it('reports reconnect and offline states', () => {
    const sources: FakeEventSource[] = [];
    const states: string[] = [];
    const stream = new EventStream({
      create: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
      onEvent: () => {},
      onState: (state) => states.push(state),
      offlineAfterMs: 2000,
    });

    stream.connect();
    sources[0].open();
    sources[0].fail();
    expect(states.at(-1)).toBe('reconnecting');

    vi.advanceTimersByTime(2000);
    expect(states.at(-1)).toBe('offline');

    sources.at(-1)!.open();
    expect(states.at(-1)).toBe('online');
    stream.close();
  });

  it('backs off exponentially and keeps the last event id', () => {
    const sources: FakeEventSource[] = [];
    const stream = new EventStream({
      create: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
      onEvent: () => {},
      onState: () => {},
      retryBaseMs: 100,
      retryMaxMs: 400,
    });

    stream.connect();
    sources[0].emit({ type: 'library_changed' });
    sources[0].fail();
    vi.advanceTimersByTime(100);
    expect(sources).toHaveLength(2);
    expect(sources[1].url).toBe('/api/events?last_event_id=1');

    sources[1].fail();
    vi.advanceTimersByTime(199);
    expect(sources).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(sources).toHaveLength(3);

    sources[2].fail();
    vi.advanceTimersByTime(400);
    expect(sources).toHaveLength(4);
    stream.close();
  });

  it('replaces the source on retry', () => {
    const sources: FakeEventSource[] = [];
    const stream = new EventStream({
      create: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
      onEvent: () => {},
      onState: () => {},
    });

    stream.connect();
    stream.retry();
    expect(sources).toHaveLength(2);
    expect(sources[0].closed).toBe(true);
    expect(sources[1].closed).toBe(false);
    stream.close();
  });

  it('reports malformed events without throwing', () => {
    const errors: unknown[] = [];
    const sources: FakeEventSource[] = [];
    const stream = new EventStream({
      create: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
      onEvent: () => {},
      onState: () => {},
      onParseError: (error) => errors.push(error),
    });
    stream.connect();
    sources[0].onmessage?.(new MessageEvent('message', { data: '{bad' }));
    expect(errors).toHaveLength(1);
    stream.close();
  });
});
