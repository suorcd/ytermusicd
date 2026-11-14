import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import AuthBanner from './AuthBanner.svelte';
import ConnectionBanner from './ConnectionBanner.svelte';
import HelpOverlay from './HelpOverlay.svelte';

describe('AuthBanner', () => {
  it('gives remediation without secrets', () => {
    render(AuthBanner, {
      props: {
        auth: {
          state: 'missing',
          source: '/home/me/.config/ytermusicd/headers.txt',
          message: 'no cookie',
        },
      },
    });

    expect(screen.getByText('YouTube Music sign-in is missing.')).toBeInTheDocument();
    expect(screen.getByText('/home/me/.config/ytermusicd/headers.txt')).toBeInTheDocument();
    expect(screen.queryByText(/SAPISID=/)).not.toBeInTheDocument();
  });

  it('shows nothing for valid auth', () => {
    const { container } = render(AuthBanner, {
      props: {
        auth: { state: 'ok', source: '/tmp/headers.txt', message: null },
      },
    });
    expect(container.textContent?.trim()).toBe('');
  });
});

describe('ConnectionBanner', () => {
  it('shows reconnect state and calls retry', async () => {
    const onretry = vi.fn();
    render(ConnectionBanner, {
      props: { connection: 'reconnecting', error: null, onretry },
    });
    expect(screen.getByText('Connection lost. Reconnecting…')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Retry now' }));
    expect(onretry).toHaveBeenCalledOnce();
  });

  it('shows the offline error', () => {
    render(ConnectionBanner, {
      props: {
        connection: 'offline',
        error: 'Daemon is unreachable. Is ytermusicd running?',
        onretry: () => {},
      },
    });
    expect(screen.getByText('The daemon is offline.')).toBeInTheDocument();
    expect(
      screen.getByText('Daemon is unreachable. Is ytermusicd running?'),
    ).toBeInTheDocument();
  });
});

describe('HelpOverlay', () => {
  it('lists keys and closes', async () => {
    const onclose = vi.fn();
    render(HelpOverlay, { props: { open: true, onclose } });
    expect(screen.getByRole('dialog', { name: 'Keyboard and mouse help' })).toBeInTheDocument();
    expect(screen.getByText('Half page down')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Close help' }));
    expect(onclose).toHaveBeenCalledOnce();
  });
});
