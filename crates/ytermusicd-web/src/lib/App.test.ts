import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import App from '../App.svelte';
import { AppStore } from '../lib/store.svelte';
import { FakeEventSource, makeApi } from '../test/fixtures';

function setup() {
  const api = makeApi();
  const sources: FakeEventSource[] = [];
  const store = new AppStore(api, {
    createEventSource: (url) => {
      const source = new FakeEventSource(url);
      sources.push(source);
      return source;
    },
  });
  return { api, store, sources };
}

describe('App shell', () => {
  it('loads the library and responds to global keys', async () => {
    const { store, sources } = setup();
    render(App, { props: { store } });

    expect(
      await screen.findByRole('heading', { level: 2, name: 'Playlist p1' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: 'Library' })).toBeInTheDocument();
    sources[0].open();
    await tick();
    expect(store.connection).toBe('online');

    await fireEvent.keyDown(window, { key: '?' });
    await tick();
    expect(
      screen.getByRole('dialog', { name: 'Keyboard and mouse help' }),
    ).toBeInTheDocument();

    await fireEvent.keyDown(window, { key: 'Escape' });
    await tick();
    expect(
      screen.queryByRole('dialog', { name: 'Keyboard and mouse help' }),
    ).not.toBeInTheDocument();

    await fireEvent.keyDown(window, { key: '/' });
    await tick();
    expect(
      screen.getByRole('dialog', { name: 'Search YouTube Music' }),
    ).toBeInTheDocument();
  });

  it('opens a playlist and shows its tracks', async () => {
    const { store } = setup();
    render(App, { props: { store } });

    store.openPlaylist('p1');
    await tick();

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Playlist p1' }),
    ).toBeInTheDocument();
    expect(await screen.findByText('Track v1')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Play all' })).toBeInTheDocument();
  });
});
