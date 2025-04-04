(function(global) {
  let spotifyReady = false;
  let spotifyToken;
  let spotifyDeviceId;
  let spotifyPlayer;

  let youtubeReady = false;
  let youtubePlayer;

  global.onSpotifyWebPlaybackSDKReady = () => {
    console.log('Spotify Player is ready');
    ws.send(JSON.stringify('SpotifyTokenRequest'));
    spotifyReady = true;
  };

  global.onYouTubeIframeAPIReady = () => {
    console.log('YouTube Player is ready');
    youtubeReady = true;
  };

  function selectSpotifyScreen() {
    document.querySelector('[data-screen]').dataset.screen = 'spotify';
    if (youtubePlayer) {
      youtubePlayer.destroy();
      youtubePlayer = undefined;
    }
  }

  function initSpotifyPlayer(accessToken) {
    selectSpotifyScreen();

    spotifyToken = accessToken;
    if (!spotifyPlayer) {
      spotifyPlayer = new Spotify.Player({
        name: 'midi-hub',
        getOAuthToken: callback => { callback(spotifyToken) },
      });

      spotifyPlayer.addListener('player_state_changed', (state) => {
        document.querySelector('.spotify-current-track__cover').src = state.track_window.current_track.album.images[0].url;
        document.querySelector('.spotify-current-track__title').textContent = state.track_window.current_track.name;
        document.querySelector('.spotify-current-track__artists').textContent = state.track_window.current_track.artists.map(artist => artist.name).join(', ');
      });

      spotifyPlayer.addListener('ready', (args) => {
        spotifyDeviceId = args.device_id;
        ws.send(JSON.stringify({ SpotifyDeviceId: { device_id: spotifyDeviceId } }));
      });

      spotifyPlayer.connect();
    }
  }

  function playSpotifyTrack(trackId, accessToken) {
    if (!spotifyReady) {
      console.log('Spotify Player is not ready yet');
      return;
    }

    initSpotifyPlayer(accessToken);

    if (spotifyDeviceId) {
      fetch(`https://api.spotify.com/v1/me/player/play?device_id=${spotifyDeviceId}`, {
        method: 'PUT',
        body: JSON.stringify({ uris: [trackId] }),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${spotifyToken}`
        },
      });
    } else {
      spotifyPlayer.addListener('ready', ({ device_id }) => {
        spotifyDeviceId = device_id;
        fetch(`https://api.spotify.com/v1/me/player/play?device_id=${spotifyDeviceId}`, {
          method: 'PUT',
          body: JSON.stringify({ uris: [trackId] }),
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${spotifyToken}`
          },
        });
      });
    }
  }

  function selectYoutubeScreen() {
    document.querySelector('[data-screen]').dataset.screen = 'youtube';
    if (spotifyPlayer) { spotifyPlayer.pause(); }
  }

  function playYoutubeVideo(videoId) {
    selectYoutubeScreen();

    if (!youtubeReady) {
      console.log('YouTube Player is not ready yet');
      return;
    }

    if (youtubePlayer) {
      youtubePlayer.destroy();
      youtubePlayer = undefined;
    }

    youtubePlayer = new YT.Player('youtube-player', {
      height: '720',
      width: '1280',
      videoId,
      events: {
        onReady: () => youtubePlayer.playVideo(),
        onStateChange: (event) => {
          if (event.data === YT.PlayerState.PAUSED || event.data === YT.PlayerState.ENDED) {
            youtubePlayer.destroy();
            youtubePlayer = undefined;
            ws.send(JSON.stringify('YoutubePause'));
          }
        },
      }
    });
  }

  const ws = new WebSocket("ws://localhost:54321/ws");
  ws.addEventListener("message", message => {
    const command = JSON.parse(message.data);
    console.log(`Received command`, command);
    if (command.SpotifyPlay) {
      playSpotifyTrack(command.SpotifyPlay.track_id, command.SpotifyPlay.access_token);
    } else if (command === 'SpotifyPause') {
      if (spotifyPlayer) {
        spotifyPlayer.pause();
      }
    } else if (command.SpotifyToken) {
      initSpotifyPlayer(command.SpotifyToken.access_token);
    } else if (command.YoutubePlay) {
      playYoutubeVideo(command.YoutubePlay.video_id);
    } else if (command === 'YoutubePause') {
      if (youtubePlayer) {
        youtubePlayer.pauseVideo();
      }
    } else {
      console.error('Unsupported command', command);
    }
  });

  document.body.addEventListener("click", () => {
    document.body.requestFullscreen();
  });
})(window);
