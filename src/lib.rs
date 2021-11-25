use std::{
    collections::HashMap,
    net::SocketAddr,
    num::{NonZeroU32, NonZeroU8},
    sync::Arc,
};

use futures_util::{SinkExt, StreamExt};
use mediasoup::{
    prelude::*,
    worker::{WorkerLogLevel, WorkerLogTag},
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::mpsc::unbounded_channel};
use tokio_tungstenite::accept_async;
use tungstenite::protocol::Message;

/// List of codecs that SFU will accept from clients.
fn media_codecs() -> Vec<RtpCodecCapability> {
    vec![
        RtpCodecCapability::Audio {
            mime_type: MimeTypeAudio::Opus,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU8::new(2).unwrap(),
            parameters: RtpCodecParametersParameters::from([("useinbandfec", 1_u32.into())]),
            rtcp_feedback: vec![RtcpFeedback::TransportCc],
        },
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::Vp8,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::default(),
            rtcp_feedback: vec![
                RtcpFeedback::Nack,
                RtcpFeedback::NackPli,
                RtcpFeedback::CcmFir,
                RtcpFeedback::GoogRemb,
                RtcpFeedback::TransportCc,
            ],
        },
    ]
}

/// Data structure containing all the necessary information about transport options required from
/// the server to establish transport connection on the client.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TransportOptions {
    id: TransportId,
    dtls_parameters: DtlsParameters,
    ice_candidates: Vec<IceCandidate>,
    ice_parameters: IceParameters,
}

#[derive(Serialize)]
#[serde(tag = "action")]
enum ServerMessage {
    /// Initialization message with consumer/producer transport options and Router's RTP
    /// capabilities necessary to establish WebRTC transport connection client-side.
    #[serde(rename_all = "camelCase")]
    Init {
        consumer_transport_options: TransportOptions,
        producer_transport_options: TransportOptions,
        router_rtp_capabilities: RtpCapabilitiesFinalized,
    },

    /// Notification that producer transport was connected successfully (in case of error connection
    /// is just dropped, in real-world application you probably want to handle it better).
    ConnectedProducerTransport,

    /// Notification that producer was created on the server, in this simple example client will try
    /// to consume it right away, hence `echo` example.
    #[serde(rename_all = "camelCase")]
    Produced { id: ProducerId },

    /// Notification that consumer transport was connected successfully (in case of error connection
    /// is just dropped, in real-world application you probably want to handle it better).
    ConnectedConsumerTransport,

    /// Notification that consumer was successfully created server-side, client can resume the
    /// consumer after this.
    #[serde(rename_all = "camelCase")]
    Consumed {
        id: ConsumerId,
        producer_id: ProducerId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    },
}

/// Client messages sent to the server.
#[derive(Deserialize)]
#[serde(tag = "action")]
enum ClientMessage {
    /// Client-side initialization with its RTP capabilities, in this simple case we expect those to
    /// match server Router's RTP capabilities
    #[serde(rename_all = "camelCase")]
    Init { rtp_capabilities: RtpCapabilities },

    /// Request to connect producer transport with client-side DTLS parameters.
    #[serde(rename_all = "camelCase")]
    ConnectProducerTransport { dtls_parameters: DtlsParameters },

    /// Request to produce a new audio or video track with specified RTP parameters.
    #[serde(rename_all = "camelCase")]
    Produce {
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    },

    /// Request to connect consumer transport with client-side DTLS parameters.
    #[serde(rename_all = "camelCase")]
    ConnectConsumerTransport { dtls_parameters: DtlsParameters },

    /// Request to consume specified producer.
    #[serde(rename_all = "camelCase")]
    Consume { producer_id: ProducerId },

    /// Request to resume consumer that was previously created.
    #[serde(rename_all = "camelCase")]
    ConsumerResume { id: ConsumerId },
}

/// Consumer/producer transports pair for the client
struct Transports {
    consumer: WebRtcTransport,
    producer: WebRtcTransport,
}

#[derive(Debug)]
enum InternalMessage {
    /// Save producer in connection-specific vec to prevent it from being destroyed.
    SaveProducer(Producer),
    /// Save consumer in connection-specific hashmap to prevent it from being destroyed.
    SaveConsumer(Consumer),
    WebsocketMessage(Message),
}

pub struct AppState {
    /// RTP capabilities received from the client.
    client_rtp_capabilities: Option<RtpCapabilities>,

    /// Consumers associated with this client, preventing them from being destroyed.
    consumers: HashMap<ConsumerId, Consumer>,

    /// Producers associated with this client, preventing them from being destroyed.
    producers: Vec<Producer>,

    /// Router associated with this client, useful to get its RTP capabilities later.
    router: Router,

    /// Consumer and producer transports associated with this client
    transports: Transports,
}

impl AppState {
    /// Create a new instance representing WebSocket connection.
    pub async fn new(worker_manager: &WorkerManager) -> Result<Self, String> {
        let mut settings = WorkerSettings::default();
        settings.log_level = WorkerLogLevel::Debug;
        settings.log_tags = vec![
            WorkerLogTag::Info,
            WorkerLogTag::Ice,
            WorkerLogTag::Dtls,
            WorkerLogTag::Rtp,
            WorkerLogTag::Srtp,
            WorkerLogTag::Rtcp,
            WorkerLogTag::Rtx,
            WorkerLogTag::Bwe,
            WorkerLogTag::Score,
            WorkerLogTag::Simulcast,
            WorkerLogTag::Svc,
            WorkerLogTag::Sctp,
            WorkerLogTag::Message,
        ];
        let worker = worker_manager
            .create_worker(settings)
            .await
            .map_err(|error| format!("Failed to create worker: {}", error))?;

        let router_options = RouterOptions::new(media_codecs());
        let router = worker
            .create_router(router_options)
            .await
            .map_err(|error| format!("Failed to create router: {}", error))?;

        // We know that for echo example we'll need 2 transports, so we can create both right away.
        // This may not be the case for real-world applications or you may create this at a
        // different time and/or in different order.
        let transport_options =
            WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
                ip: "127.0.0.1".parse().unwrap(),
                announced_ip: None,
            }));

        let producer_transport = router
            .create_webrtc_transport(transport_options.clone())
            .await
            .map_err(|error| format!("Failed to create producer transport: {}", error))?;

        let consumer_transport = router
            .create_webrtc_transport(transport_options)
            .await
            .map_err(|error| format!("Failed to create consumer transport: {}", error))?;

        Ok(Self {
            client_rtp_capabilities: None,
            consumers: HashMap::new(),
            producers: vec![],
            router,
            transports: Transports {
                consumer: consumer_transport,
                producer: producer_transport,
            },
        })
    }

    fn save_producer_or_consumer(&mut self, message: InternalMessage) {
        match message {
            InternalMessage::SaveProducer(producer) => {
                self.producers.push(producer);
            }
            InternalMessage::SaveConsumer(consumer) => {
                self.consumers.insert(consumer.id(), consumer);
            }
            _ => (),
        }
    }
}

pub async fn handle_connection(
    peer: SocketAddr,
    stream: TcpStream,
    worker_manager: Arc<WorkerManager>,
) {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
    println!("New WebSocket connection: {}", peer);

    let mut state = AppState::new(&worker_manager)
        .await
        .expect("Could not create app state.");

    let (mut sender, mut receiver) = ws_stream.split();

    let (tx, mut rx) = unbounded_channel();

    tokio::task::spawn_local(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                InternalMessage::WebsocketMessage(msg) => {
                    sender.send(msg).await.unwrap();
                }
                // InternalMessage::SaveProducer(producer) => {
                //     // state.producers.push(producer);
                // }
                // InternalMessage::SaveConsumer(consumer) => {
                //     // state.consumers.insert(consumer.id(), consumer);
                // }
                _ => state.save_producer_or_consumer(msg),
            }
        }
    });

    // We know that both consumer and producer transports will be used, so we sent server
    // information about both in an initialization message alongside with router capabilities
    // to the client right after WebSocket connection is established.
    let server_init_message = ServerMessage::Init {
        consumer_transport_options: TransportOptions {
            id: state.transports.consumer.id(),
            dtls_parameters: state.transports.consumer.dtls_parameters(),
            ice_candidates: state.transports.consumer.ice_candidates().clone(),
            ice_parameters: state.transports.consumer.ice_parameters().clone(),
        },
        producer_transport_options: TransportOptions {
            id: state.transports.producer.id(),
            dtls_parameters: state.transports.producer.dtls_parameters(),
            ice_candidates: state.transports.producer.ice_candidates().clone(),
            ice_parameters: state.transports.producer.ice_parameters().clone(),
        },
        router_rtp_capabilities: state.router.rtp_capabilities().clone(),
    };

    let msg = serde_json::to_string(&server_init_message).unwrap();
    // sender.send(Message::Text(msg)).await.unwrap();
    tx.send(InternalMessage::WebsocketMessage(Message::Text(msg)))
        .unwrap();

    while let Some(Ok(Message::Text(message))) = receiver.next().await {
        let msg = serde_json::from_str::<ClientMessage>(&message).unwrap();
        match msg {
            ClientMessage::Init { rtp_capabilities } => {
                // We need to know client's RTP capabilities, those are sent using initialization
                // message and are stored in connection struct for future use.
                state.client_rtp_capabilities.replace(rtp_capabilities);
            }
            ClientMessage::ConnectProducerTransport { dtls_parameters } => {
                let transport = state.transports.producer.clone();

                // Establish connection for producer transport using DTLS parameters received
                // from the client, but doing so in a background task since this handler is
                // synchronous.
                let tx = tx.clone();
                tokio::task::spawn_local(async move {
                    match transport
                        .connect(WebRtcTransportRemoteParameters { dtls_parameters })
                        .await
                    {
                        Ok(_) => {
                            let msg =
                                serde_json::to_string(&ServerMessage::ConnectedProducerTransport)
                                    .unwrap();
                            // sender.send(Message::Text(msg)).await.unwrap();
                            tx.send(InternalMessage::WebsocketMessage(Message::Text(msg)))
                                .unwrap();
                            println!("Producer transport connected");
                        }
                        Err(error) => {
                            eprintln!("Failed to connect producer transport: {}", error);
                        }
                    }
                });
            }
            ClientMessage::Produce {
                kind,
                rtp_parameters,
            } => {
                let transport = state.transports.producer.clone();

                // Use producer transport to create a new producer on the server with given RTP
                // parameters.
                let tx = tx.clone();
                tokio::task::spawn_local(async move {
                    match transport
                        .produce(ProducerOptions::new(kind, rtp_parameters))
                        .await
                    {
                        Ok(producer) => {
                            let id = producer.id();
                            let msg =
                                serde_json::to_string(&ServerMessage::Produced { id }).unwrap();
                            // sender.send(Message::Text(msg)).await.unwrap();
                            tx.send(InternalMessage::WebsocketMessage(Message::Text(msg)))
                                .unwrap();

                            // Retain producer to prevent it from being destroyed.
                            // Producer is stored in a vec since if we don't do it, it will get
                            // destroyed as soon as its instance goes out out scope
                            // state.producers.push(producer);
                            tx.send(InternalMessage::SaveProducer(producer)).unwrap();

                            println!("{:?} producer created: {}", kind, id);
                        }
                        Err(error) => {
                            eprintln!("Failed to create {:?} producer: {}", kind, error);
                        }
                    }
                });
            }
            ClientMessage::ConnectConsumerTransport { dtls_parameters } => {
                let transport = state.transports.consumer.clone();

                // The same as producer transport, but for consumer transport.
                let tx = tx.clone();
                tokio::task::spawn_local(async move {
                    match transport
                        .connect(WebRtcTransportRemoteParameters { dtls_parameters })
                        .await
                    {
                        Ok(_) => {
                            let msg =
                                serde_json::to_string(&ServerMessage::ConnectedConsumerTransport)
                                    .unwrap();
                            // sender.send(Message::Text(msg)).await.unwrap();
                            tx.send(InternalMessage::WebsocketMessage(Message::Text(msg)))
                                .unwrap();

                            println!("Consumer transport connected");
                        }
                        Err(error) => {
                            eprintln!("Failed to connect consumer transport: {}", error);
                        }
                    }
                });
            }
            ClientMessage::Consume { producer_id } => {
                let transport = state.transports.consumer.clone();
                let rtp_capabilities = match state.client_rtp_capabilities.clone() {
                    Some(rtp_capacities) => rtp_capacities,
                    None => {
                        eprintln!("Client should send RTP capabilities before consuming");
                        return;
                    }
                };

                // Create consumer for given producer ID, while first making sure that RTP
                // capabilities were sent by the client prior to that.
                let mut options = ConsumerOptions::new(producer_id, rtp_capabilities);
                options.paused = true;

                let tx = tx.clone();
                tokio::task::spawn_local(async move {
                    match transport.consume(options).await {
                        Ok(consumer) => {
                            let id = consumer.id();
                            let kind = consumer.kind();
                            let rtp_parameters = consumer.rtp_parameters().clone();

                            let msg = serde_json::to_string(&ServerMessage::Consumed {
                                id,
                                producer_id,
                                kind,
                                rtp_parameters,
                            })
                            .unwrap();
                            // sender.send(Message::Text(msg)).await.unwrap();
                            tx.send(InternalMessage::WebsocketMessage(Message::Text(msg)))
                                .unwrap();

                            // state.consumers.insert(consumer.id(), consumer);
                        }
                        Err(error) => {
                            eprintln!("Failed to create consumer: {}", error);
                        }
                    }
                });
            }
            ClientMessage::ConsumerResume { id } => {
                if let Some(consumer) = state.consumers.get(&id).cloned() {
                    tokio::task::spawn(async move {
                        match consumer.resume().await {
                            Ok(_) => {
                                println!(
                                    "Successfully resumed {:?} consumer {}",
                                    consumer.kind(),
                                    consumer.id(),
                                );
                            }
                            Err(error) => {
                                println!(
                                    "Failed to resume {:?} consumer {}: {}",
                                    consumer.kind(),
                                    consumer.id(),
                                    error,
                                );
                            }
                        }
                    });
                }
            }
        }
    }
}
