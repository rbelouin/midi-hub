use tokio::runtime::Builder;
use warp::Filter;

pub struct HttpServer {}

impl HttpServer {
    pub fn start() -> Self {
        std::thread::spawn(move || {
            Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let public = warp::any()
                        .and(warp::fs::dir("public"));

                    println!("HTTP server listening on http://localhost:54321/");
                    warp::serve(public)
                        .run(([0, 0, 0, 0], 54321))
                        .await;
                });
        });

        HttpServer {}
    }
}
