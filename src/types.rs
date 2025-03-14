// This file is @generated by prost-build.
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VoiceStreamResponse {}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VoiceStreamRequest {
    /// This is a placeholder comment.
    #[prost(oneof = "voice_stream_request::Request", tags = "1, 2")]
    pub request: ::core::option::Option<voice_stream_request::Request>,
}
/// Nested message and enum types in `VoiceStreamRequest`.
pub mod voice_stream_request {
    /// This is a placeholder comment.
    #[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
    #[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        #[prost(string, tag = "1")]
        AgentId(::prost::alloc::string::String),
        #[prost(bytes, tag = "2")]
        Data(::prost::alloc::vec::Vec<u8>),
    }
}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UserEvent {
    #[prost(message, optional, tag = "1")]
    pub user_id: ::core::option::Option<::flux::prelude::Thing>,
    #[prost(message, optional, tag = "2")]
    pub space_id: ::core::option::Option<::flux::prelude::Thing>,
    #[prost(message, optional, tag = "3")]
    pub context_id: ::core::option::Option<::flux::prelude::Thing>,
    /// This is a placeholder comment.
    #[prost(
        oneof = "user_event::UserEventType",
        tags = "100, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113"
    )]
    pub user_event_type: ::core::option::Option<user_event::UserEventType>,
}
/// Nested message and enum types in `UserEvent`.
pub mod user_event {
    /// This is a placeholder comment.
    #[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
    #[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum UserEventType {
        #[prost(message, tag = "100")]
        SpeakBytesEvent(super::SpeakBytesEvent),
        #[prost(message, tag = "102")]
        SpeakEvent(super::SpeakEvent),
        #[prost(message, tag = "103")]
        SpeakResultEvent(super::SpeakResultEvent),
        #[prost(message, tag = "104")]
        UserJoinedEvent(super::UserJoinedEvent),
        #[prost(message, tag = "105")]
        UserLeftEvent(super::UserLeftEvent),
        #[prost(message, tag = "106")]
        ResetDeviceEvent(super::ResetDeviceEvent),
        #[prost(message, tag = "107")]
        WaitEvent(super::WaitEvent),
        #[prost(message, tag = "108")]
        SingEvent(super::SingEvent),
        #[prost(message, tag = "109")]
        SleepEvent(super::SleepEvent),
        #[prost(message, tag = "110")]
        WakeEvent(super::WakeEvent),
        #[prost(message, tag = "111")]
        SystemEvent(super::SystemEvent),
        #[prost(message, tag = "112")]
        ImageBytesEvent(super::ImageBytesEvent),
        #[prost(message, tag = "113")]
        EmoteEvent(super::EmoteEvent),
    }
}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpeakBytesEvent {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, optional, tag = "2")]
    pub text: ::core::option::Option<::prost::alloc::string::String>,
}
/// / Speaks text using the provided voice name and emotion. The text may be a single sentence or multiple sentences.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpeakEvent {
    #[prost(string, tag = "1")]
    pub text: ::prost::alloc::string::String,
}
/// / Sets the current emotion of the agent. Call this function prior to speaking if the tone of the agent's voice should be different than the last emotion of the agent.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EmoteEvent {
    #[prost(string, tag = "1")]
    pub text: ::prost::alloc::string::String,
}
/// / Sings a song with the name provided. Must be one of the songs specified as available, if any.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SingEvent {
    #[prost(string, tag = "1")]
    pub song_name: ::prost::alloc::string::String,
}
/// Puts the agent to sleep. Call this function if a user requests the agent to be turned off.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SleepEvent {}
/// Awakes the agent from sleep. Call this function if a user requests the agent to be turned on after being turned off. The user should specifically say the agent's name for this to be called.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WakeEvent {}
/// This represents a general system message sent to an agent. An agent will receive system messages if there is some general information it needs to be made aware of.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SystemEvent {
    #[prost(string, tag = "1")]
    pub message: ::prost::alloc::string::String,
}
/// This represents no response in a conversation. Call this function if no function should be called. Used instead of any other functions if it is most appropriate to wait for further outside input instead of responding. ONLY use this if explicitly waiting for input from a player.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WaitEvent {}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ImageBytesEvent {
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpeakResultEvent {
    #[prost(string, tag = "1")]
    pub asset_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub text: ::prost::alloc::string::String,
}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UserJoinedEvent {}
/// This is a placeholder comment.
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UserLeftEvent {}
/// This action resets the device the agent is running on. Only use this action if prompted to!
#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]
#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResetDeviceEvent {}
/// Generated client implementations.
pub mod agent_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Interface exported by the server.
    #[derive(Debug, Clone)]
    pub struct AgentServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl<T> AgentServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> AgentServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            AgentServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn stream_voice(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::VoiceStreamRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<super::VoiceStreamResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ragent.AgentService/StreamVoice",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ragent.AgentService", "StreamVoice"));
            self.inner.client_streaming(req, path, codec).await
        }
    }
}

/*
/// Generated server implementations.
pub mod agent_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with AgentServiceServer.
    #[async_trait]
    pub trait AgentService: Send + Sync + 'static {
        async fn stream_voice(
            &self,
            request: tonic::Request<tonic::Streaming<super::VoiceStreamRequest>>,
        ) -> std::result::Result<
            tonic::Response<super::VoiceStreamResponse>,
            tonic::Status,
        >;
    }
    /// Interface exported by the server.
    #[derive(Debug)]
    pub struct AgentServiceServer<T: AgentService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: AgentService> AgentServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for AgentServiceServer<T>
    where
        T: AgentService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/ragent.AgentService/StreamVoice" => {
                    #[allow(non_camel_case_types)]
                    struct StreamVoiceSvc<T: AgentService>(pub Arc<T>);
                    impl<
                        T: AgentService,
                    > tonic::server::ClientStreamingService<super::VoiceStreamRequest>
                    for StreamVoiceSvc<T> {
                        type Response = super::VoiceStreamResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<
                                tonic::Streaming<super::VoiceStreamRequest>,
                            >,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as AgentService>::stream_voice(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = StreamVoiceSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.client_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: AgentService> Clone for AgentServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: AgentService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: AgentService> tonic::server::NamedService for AgentServiceServer<T> {
        const NAME: &'static str = "ragent.AgentService";
    }
}
*/