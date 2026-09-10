use deepgram::{
    Deepgram,
    common::options::{DetectLanguage, Encoding, Endpointing, Model, Options},
    listen::websocket::WebsocketBuilder,
};

pub(super) fn standard_request(
    dg: &Deepgram,
    model: Model,
    sample_rate: u32,
) -> WebsocketBuilder<'_> {
    dg.transcription()
        .stream_request_with_options(
            Options::builder()
                .model(model)
                .detect_language(DetectLanguage::Disabled)
                .build(),
        )
        .encoding(Encoding::Linear16)
        .sample_rate(sample_rate)
        .channels(1)
        .endpointing(Endpointing::CustomDurationMs(300))
        .interim_results(true)
        // The pinned SDK's worker can park forever after 3s without audio
        // when keepalive is disabled. HTTP upload reconnects can exceed that.
        .keep_alive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepgram::common::stream_response::StreamResponse;
    use futures::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio::{net::TcpListener, time::timeout};
    use tokio_tungstenite::{accept_hdr_async, tungstenite::Message};

    /// No credentials or external service: exercise the actual pinned SDK
    /// across an upload pause longer than its three-second idle timer.
    #[tokio::test]
    async fn audio_resumes_after_idle_and_session_closes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = accept_hdr_async(socket, |_: &_, mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                response.headers_mut().insert("dg-request-id", "550e8400-e29b-41d4-a716-446655440000".parse().unwrap());
                Ok(response)
            }).await.unwrap();
            let mut audio_chunks = 0;
            let mut keepalives = 0;
            while let Some(message) = ws.next().await {
                match message.unwrap() {
                    Message::Binary(_) => {
                        audio_chunks += 1;
                        if audio_chunks == 2 {
                            ws.send(Message::Text(r#"{"type":"Results","start":0.0,"duration":0.04,"is_final":true,"speech_final":true,"from_finalize":false,"channel":{"alternatives":[{"transcript":"hello again","words":[],"confidence":1.0}]},"metadata":{"request_id":"test","model_info":{"name":"nova-3","version":"test","arch":"test"},"model_uuid":"test"},"channel_index":[0,1]}"#.into())).await.unwrap();
                        }
                    }
                    Message::Text(text) if text.contains("KeepAlive") => keepalives += 1,
                    Message::Text(text) if text.contains("CloseStream") => {
                        assert_eq!(audio_chunks, 2);
                        assert!(keepalives > 0);
                        ws.send(Message::Text(r#"{"request_id":"test","created":"test","duration":0.04,"channels":1}"#.into())).await.unwrap();
                        ws.close(None).await.unwrap();
                        return;
                    }
                    _ => {}
                }
            }
            panic!("session ended without CloseStream");
        });
        let base_url = format!("http://{address}");
        let dg = Deepgram::with_base_url_and_api_key(base_url.as_str(), "test").unwrap();
        let (mut audio_tx, audio_rx) =
            futures::channel::mpsc::channel::<Result<_, std::io::Error>>(16);
        let mut results = standard_request(&dg, Model::Nova3, 16000)
            .stream(audio_rx)
            .await
            .unwrap();
        audio_tx.send(Ok(vec![0; 640].into())).await.unwrap();
        tokio::time::sleep(Duration::from_millis(3300)).await;
        audio_tx.send(Ok(vec![0; 640].into())).await.unwrap();
        let response = timeout(Duration::from_secs(2), results.next())
            .await
            .expect("SDK stopped receiving after upload pause")
            .unwrap()
            .unwrap();
        assert!(matches!(
            response,
            StreamResponse::TranscriptResponse { is_final: true, .. }
        ));
        // This is how the transcriber ends an idle or cancelled input task.
        drop(audio_tx);
        let response = timeout(Duration::from_secs(2), results.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(response, StreamResponse::TerminalResponse { .. }));
        timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }
}
