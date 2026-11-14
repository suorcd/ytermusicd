import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Keymap, isTypingTarget, type KeymapActions } from './keymap';

function actions(): KeymapActions {
  return {
    move: vi.fn(),
    first: vi.fn(),
    last: vi.fn(),
    halfPage: vi.fn(),
    openSearch: vi.fn(),
    playSelected: vi.fn(),
    togglePlayback: vi.fn(),
    queueSelected: vi.fn(),
    deleteSelected: vi.fn(),
    openHelp: vi.fn(),
    closeOverlays: vi.fn(),
  };
}

function key(value: string, init: KeyboardEventInit = {}) {
  return new KeyboardEvent('keydown', { key: value, cancelable: true, ...init });
}

describe('keymap', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('handles vim movement and jumps', () => {
    const handler = actions();
    const keymap = new Keymap(handler);

    expect(keymap.handle(key('j'))).toBe(true);
    expect(handler.move).toHaveBeenLastCalledWith(1);
    expect(keymap.handle(key('k'))).toBe(true);
    expect(handler.move).toHaveBeenLastCalledWith(-1);

    keymap.handle(key('g'));
    keymap.handle(key('g'));
    expect(handler.first).toHaveBeenCalledOnce();
    expect(handler.move).toHaveBeenCalledTimes(2);

    keymap.handle(key('G'));
    expect(handler.last).toHaveBeenCalledOnce();
  });

  it('expires the gg sequence', () => {
    const handler = actions();
    const keymap = new Keymap(handler);
    keymap.handle(key('g'));
    vi.advanceTimersByTime(700);
    keymap.handle(key('g'));
    expect(handler.first).not.toHaveBeenCalled();
  });

  it('handles dd and ignores a single d', () => {
    const handler = actions();
    const keymap = new Keymap(handler);
    keymap.handle(key('d'));
    expect(handler.deleteSelected).not.toHaveBeenCalled();
    keymap.handle(key('d'));
    expect(handler.deleteSelected).toHaveBeenCalledOnce();
  });

  it('handles control shortcuts', () => {
    const handler = actions();
    const keymap = new Keymap(handler);
    keymap.handle(key('d', { ctrlKey: true }));
    expect(handler.halfPage).toHaveBeenLastCalledWith(1);
    keymap.handle(key('u', { ctrlKey: true }));
    expect(handler.halfPage).toHaveBeenLastCalledWith(-1);
    keymap.handle(key('k', { ctrlKey: true }));
    expect(handler.openSearch).toHaveBeenCalledOnce();
  });

  it('handles actions and help', () => {
    const handler = actions();
    const keymap = new Keymap(handler);
    keymap.handle(key('/'));
    keymap.handle(key('?'));
    keymap.handle(key('Enter'));
    keymap.handle(key(' '));
    keymap.handle(key('x'));
    keymap.handle(key('Escape'));

    expect(handler.openSearch).toHaveBeenCalledOnce();
    expect(handler.openHelp).toHaveBeenCalledOnce();
    expect(handler.playSelected).toHaveBeenCalledOnce();
    expect(handler.togglePlayback).toHaveBeenCalledOnce();
    expect(handler.queueSelected).toHaveBeenCalledOnce();
    expect(handler.closeOverlays).toHaveBeenCalledOnce();
  });

  it('ignores shortcuts while typing', () => {
    const handler = actions();
    const keymap = new Keymap(handler);
    const input = document.createElement('input');

    const handled = keymap.handle(
      new KeyboardEvent('keydown', { key: 'j', bubbles: true, cancelable: true }),
      { typing: true },
    );
    expect(handled).toBe(false);
    expect(handler.move).not.toHaveBeenCalled();

    const event = new KeyboardEvent('keydown', { key: 'j', cancelable: true });
    Object.defineProperty(event, 'target', { value: input });
    expect(keymap.handle(event)).toBe(false);
    expect(isTypingTarget(input)).toBe(true);
  });

  it('leaves overlay keys to the overlay', () => {
    const handler = actions();
    const keymap = new Keymap(handler);
    expect(keymap.handle(key('j'), { overlayOpen: true })).toBe(false);
    expect(handler.move).not.toHaveBeenCalled();
    expect(keymap.handle(key('Escape'), { overlayOpen: true })).toBe(true);
    expect(handler.closeOverlays).toHaveBeenCalledOnce();
  });
});
