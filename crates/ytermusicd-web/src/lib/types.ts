import type { Playlist } from '@api/Playlist';
import type { Track } from '@api/Track';

export interface MenuItem {
  label: string;
  disabled?: boolean;
  action: () => void;
}

export type SearchEntry =
  | { kind: 'track'; track: Track }
  | { kind: 'playlist'; playlist: Playlist };
