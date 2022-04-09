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
    setTimeout(() => playVideo('Dy-WpCFz1j4'), 1000);
    setTimeout(() => playVideo('w2sF0Gn4UcQ'), 9000);
  });
}
