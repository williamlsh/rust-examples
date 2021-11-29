use crate::participant::ParticipantId;
use crate::room::RoomId;
use actix::prelude::*;
use mediasoup::prelude::*;
use serde::{Deserialize, Serialize};

/// Data structure containing all the necessary information about transport options required
/// from the server to establish transport connection on the client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportOptions {
    pub id: TransportId,
    pub dtls_parameters: DtlsParameters,
    pub ice_candidates: Vec<IceCandidate>,
    pub ice_parameters: IceParameters,
}

/// Server messages sent to the client
#[derive(Serialize, Message)]
#[serde(tag = "action")]
#[rtype(result = "()")]
pub enum ServerMessage {
    /// Initialization message with consumer/producer transport options and Router's RTP
    /// capabilities necessary to establish WebRTC transport connection client-side
    #[serde(rename_all = "camelCase")]
    Init {
        room_id: RoomId,
        consumer_transport_options: TransportOptions,
        producer_transport_options: TransportOptions,
        router_rtp_capabilities: RtpCapabilitiesFinalized,
    },
    /// Notification that new producer was added to the room
    #[serde(rename_all = "camelCase")]
    ProducerAdded {
        participant_id: ParticipantId,
        producer_id: ProducerId,
    },
    /// Notification that producer was removed from the room
    #[serde(rename_all = "camelCase")]
    ProducerRemoved {
        participant_id: ParticipantId,
        producer_id: ProducerId,
    },
    /// Notification that producer transport was connected successfully (in case of error
    /// connection is just dropped, in real-world application you probably want to handle it
    /// better)
    ConnectedProducerTransport,
    /// Notification that producer was created on the server
    #[serde(rename_all = "camelCase")]
    Produced { id: ProducerId },
    /// Notification that consumer transport was connected successfully (in case of error
    /// connection is just dropped, in real-world application you probably want to handle it
    /// better)
    ConnectedConsumerTransport,
    /// Notification that consumer was successfully created server-side, client can resume
    /// the consumer after this
    #[serde(rename_all = "camelCase")]
    Consumed {
        id: ConsumerId,
        producer_id: ProducerId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    },
}

/// Client messages sent to the server
#[derive(Deserialize, Message)]
#[serde(tag = "action")]
#[rtype(result = "()")]
pub enum ClientMessage {
    /// Client-side initialization with its RTP capabilities, in this simple case we expect
    /// those to match server Router's RTP capabilities
    #[serde(rename_all = "camelCase")]
    Init { rtp_capabilities: RtpCapabilities },
    /// Request to connect producer transport with client-side DTLS parameters
    #[serde(rename_all = "camelCase")]
    ConnectProducerTransport { dtls_parameters: DtlsParameters },
    /// Request to produce a new audio or video track with specified RTP parameters
    #[serde(rename_all = "camelCase")]
    Produce {
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    },
    /// Request to connect consumer transport with client-side DTLS parameters
    #[serde(rename_all = "camelCase")]
    ConnectConsumerTransport { dtls_parameters: DtlsParameters },
    /// Request to consume specified producer
    #[serde(rename_all = "camelCase")]
    Consume { producer_id: ProducerId },
    /// Request to resume consumer that was previously created
    #[serde(rename_all = "camelCase")]
    ConsumerResume { id: ConsumerId },
}

/// Internal actor messages for convenience
#[derive(Message)]
#[rtype(result = "()")]
pub enum InternalMessage {
    /// Save producer in connection-specific hashmap to prevent it from being destroyed
    SaveProducer(Producer),
    /// Save consumer in connection-specific hashmap to prevent it from being destroyed
    SaveConsumer(Consumer),
    /// Stop/close the WebSocket connection
    Stop,
}
