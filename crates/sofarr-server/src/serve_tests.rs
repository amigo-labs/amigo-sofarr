//! The server's shutdown path stops a running encode (spec reencode-job AC-06).

use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use sofarr_db::ReencodeOutcome;
use sofarr_mediainfo::Container;

use crate::reencode::{
    ffmpeg::{EncodeError, Job},
    run::{
        self,
        tests::{Behavior, Env, hevc, info, write_fake},
    },
};

#[tokio::test]
async fn shutting_the_server_down_stops_a_running_encode() {
    let env = Env::new("serve-stop", "live").await;
    let (id, _) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let waits: Behavior = Box::new(|job: &Job| {
        write_fake(&job.output, &hevc(), 1000);
        let cancel = job.cancel.clone().expect("a stop flag");
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(EncodeError::Cancelled)
    });
    let (tools, _) = env.tools(Ok(()), waits);
    let tools = Arc::new(tools);
    *env.app.reencode.tools.lock().expect("tools") = Some(Arc::clone(&tools));

    // The server runs; an encode started through it is under way.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let (signal, received) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(super::serve_until(Arc::clone(&env.app), listener, async {
        let _ = received.await;
    }));
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let answer = client
        .post(format!("http://{address}/api/reencode/encode"))
        .json(&serde_json::json!({ "movie_id": id }))
        .send()
        .await
        .expect("request");
    assert_eq!(answer.status().as_u16(), 202);
    while env.app.reencode.in_flight.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // The signal: the server stops, and with it the encoder.
    let started = Instant::now();
    signal.send(()).expect("signal");
    server.await.expect("server task").expect("served");
    assert!(started.elapsed() < Duration::from_secs(10));
    let last = env
        .db()
        .last_reencode()
        .await
        .expect("read")
        .expect("attempt");
    assert_eq!(last.outcome, Some(ReencodeOutcome::Failed));
    assert_eq!(
        last.reason.as_deref(),
        Some("abgebrochen: sofarr wurde beendet")
    );
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    assert!(run::running(&env.app.reencode.lock).is_none());
}
