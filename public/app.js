function onYouTubeIframeAPIReady() {
  console.log("YouTube iframe API is ready");

  let player;

  function playVideo(videoId) {
    console.log(`Playing video ${videoId}`);

    if (player) {
      player.loadVideoById(videoId, 0, 'hd720');
    } else {
      player = new YT.Player('youtube-player', {
        height: '720',
        width: '1280',
        videoId,
        events: {
          onReady: () => player.playVideo(),
          onStateChange: (event) => {
            if (event.data === YT.PlayerState.ENDED) {
              player.destroy();
              player = undefined;
            }
          },
        }
      });
    }
  }

  document.body.addEventListener("click", () => {
    document.body.requestFullscreen();
  });

  const ws = new WebSocket("ws://localhost:54321/ws");
  ws.addEventListener("message", message => {
    const command = JSON.parse(message.data);
    if (command.Play) {
      playVideo(command.Play);
    } else {
      console.error('Unsupported command', command);
    }
  });
}
