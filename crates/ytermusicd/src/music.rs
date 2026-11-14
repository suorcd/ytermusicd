use async_trait::async_trait;
use std::sync::Arc;
use ytermusicd_api::{Playlist, SearchResults, Track};
use ytpapi2::{Endpoint, YoutubeMusicInstance, YoutubeMusicPlaylistRef};

#[async_trait]
#[allow(clippy::double_must_use)]
pub trait MusicApi: Send + Sync {
    async fn playlists(&self) -> Result<Vec<Playlist>, String>;
    async fn playlist(&self, id: &str) -> Result<Vec<Track>, String>;
    async fn search(&self, query: &str) -> Result<SearchResults, String>;
}

pub struct YoutubeMusicApi {
    client: Arc<YoutubeMusicInstance>,
}

impl YoutubeMusicApi {
    pub fn new(client: YoutubeMusicInstance) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

#[async_trait]
impl MusicApi for YoutubeMusicApi {
    async fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let mut playlists = Vec::new();
        for endpoint in [Endpoint::MusicLibraryLanding, Endpoint::MusicLikedPlaylists] {
            match self.client.get_library(&endpoint, 2).await {
                Ok(found) => playlists.extend(found.into_iter().map(playlist_dto)),
                Err(error) => log::warn!("library endpoint {endpoint:?} failed: {error:?}"),
            }
        }
        playlists.sort_by(|a, b| a.name.cmp(&b.name));
        playlists.dedup_by(|a, b| a.id == b.id);
        if playlists.is_empty() {
            // Distinguish a genuinely empty library from a total failure.
            return Err("could not load playlists from YouTube Music".to_string());
        }
        Ok(playlists)
    }

    async fn playlist(&self, id: &str) -> Result<Vec<Track>, String> {
        self.client
            .get_playlist_raw(id, 5)
            .await
            .map(|tracks| tracks.into_iter().map(track_dto).collect())
            .map_err(|error| format!("failed to load playlist {id}: {error:?}"))
    }

    async fn search(&self, query: &str) -> Result<SearchResults, String> {
        self.client
            .search(query, 2)
            .await
            .map(|results| SearchResults {
                tracks: results.videos.into_iter().map(track_dto).collect(),
                playlists: results.playlists.into_iter().map(playlist_dto).collect(),
            })
            .map_err(|error| format!("search failed: {error:?}"))
    }
}

pub struct UnavailableMusicApi;

#[async_trait]
impl MusicApi for UnavailableMusicApi {
    async fn playlists(&self) -> Result<Vec<Playlist>, String> {
        Err("YouTube Music client is unavailable; check auth/network".to_string())
    }

    async fn playlist(&self, _id: &str) -> Result<Vec<Track>, String> {
        Err("YouTube Music client is unavailable; check auth/network".to_string())
    }

    async fn search(&self, _query: &str) -> Result<SearchResults, String> {
        Err("YouTube Music client is unavailable; check auth/network".to_string())
    }
}

pub fn playlist_dto(playlist: YoutubeMusicPlaylistRef) -> Playlist {
    Playlist {
        id: playlist.browse_id,
        name: playlist.name,
        subtitle: playlist.subtitle,
    }
}

pub fn track_dto(track: ytpapi2::YoutubeMusicVideoRef) -> Track {
    Track {
        video_id: track.video_id,
        title: track.title,
        artist: track.author,
        album: track.album,
        duration: track.duration,
    }
}

pub fn core_track(track: Track) -> ytermusicd_core::Track {
    ytermusicd_core::Track {
        video_id: track.video_id,
        title: track.title,
        author: track.artist,
        album: track.album,
        duration: track.duration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_track_and_playlist() {
        let track = track_dto(ytpapi2::YoutubeMusicVideoRef {
            video_id: "v1".to_string(),
            title: "Song".to_string(),
            author: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        });
        assert_eq!(track.artist, "Artist");
        let core = core_track(track);
        assert_eq!(core.author, "Artist");

        let playlist = playlist_dto(YoutubeMusicPlaylistRef {
            name: "Mix".to_string(),
            subtitle: "Artist".to_string(),
            browse_id: "VL1".to_string(),
        });
        assert_eq!(playlist.id, "VL1");
    }
}
