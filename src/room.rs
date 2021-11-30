use crate::participant::ParticipantId;
use event_listener_primitives::{Bag, BagOnce, HandlerId};
use mediasoup::prelude::*;
use mediasoup::worker::{WorkerLogLevel, WorkerLogTag};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::num::{NonZeroU32, NonZeroU8};
use std::sync::{Arc, Weak};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct RoomId(Uuid);

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl RoomId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Default)]
struct Handlers {
    producer_add:
        Bag<Arc<dyn Fn(&ParticipantId, &Producer) + Send + Sync>, ParticipantId, Producer>,
    producer_remove:
        Bag<Arc<dyn Fn(&ParticipantId, &ProducerId) + Send + Sync>, ParticipantId, ProducerId>,
    close: BagOnce<Box<dyn FnOnce() + Send>>,
}

struct Inner {
    id: RoomId,
    router: Router,
    handlers: Handlers,
    clients: Mutex<HashMap<ParticipantId, Vec<Producer>>>,
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner")
            .field("id", &self.id)
            .field("handlers", &"...")
            .field("clients", &self.clients)
            .finish()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        println!("Room {} closed", self.id);

        self.handlers.close.call_simple();
    }
}

/// Room holds producers of the participants such that other participants can consume audio and
/// video tracks of each other.
#[derive(Debug, Clone)]
pub struct Room {
    inner: Arc<Inner>,
}

impl Room {
    pub async fn new(worker_manager: &WorkerManager) -> Result<Self, String> {
        Self::new_with_id(worker_manager, RoomId::new()).await
    }

    pub async fn new_with_id(worker_manager: &WorkerManager, id: RoomId) -> Result<Room, String> {
        let worker = worker_manager
            .create_worker({
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
                settings
            })
            .await
            .map_err(|error| format!("Failed to create worker: {}", error))?;

        let router = worker
            .create_router(RouterOptions::new(self::media_codecs()))
            .await
            .map_err(|error| format!("Failed to create router: {}", error))?;

        println!("Room {} created", id);

        Ok(Self {
            inner: Arc::new(Inner {
                id,
                router,
                handlers: Handlers::default(),
                clients: Mutex::default(),
            }),
        })
    }

    pub fn id(&self) -> RoomId {
        self.inner.id
    }

    pub fn router(&self) -> &Router {
        &self.inner.router
    }

    /// Add producer to the room, this will trigger notifications to other participants that
    /// will be able to consume it.
    ///
    /// One participant has two producers, video and audio producers.
    pub fn add_producer(&self, participant_id: ParticipantId, producer: Producer) {
        self.inner
            .clients
            .lock()
            .entry(participant_id)
            .or_default()
            .push(producer.clone());

        // Call all registered callback handlers to notify new producer.
        self.inner
            .handlers
            .producer_add
            .call_simple(&participant_id, &producer);
    }

    /// Remove participant and all of its associated producers.
    pub fn remove_participant(&self, participant_id: &ParticipantId) {
        let producers = self.inner.clients.lock().remove(participant_id);

        // Call all registered callback handlers.
        for producer in producers.unwrap_or_default() {
            let producer_id = &producer.id();
            self.inner
                .handlers
                .producer_remove
                .call_simple(participant_id, producer_id);
        }
    }

    /// Get all producers of all participants, useful when new participant connects and needs to
    /// consume tracks of everyone who is already in the room.
    pub fn get_all_producers(&self) -> Vec<(ParticipantId, ProducerId)> {
        self.inner
            .clients
            .lock()
            .iter()
            .map(|(participant_id, producers)| {
                let participant_id = *participant_id;
                producers
                    .iter()
                    .map(move |producer| (participant_id, producer.id()))
            })
            .flatten()
            .collect()
    }

    /// Subscribe to notifications when new producer is added to the room.
    pub fn on_producer_add<F: Fn(&ParticipantId, &Producer) + Send + Sync + 'static>(
        &self,
        callback: F,
    ) -> HandlerId {
        self.inner.handlers.producer_add.add(Arc::new(callback))
    }

    /// Subscribe to notifications when producer is removed from the room.
    pub fn on_producer_remove<F: Fn(&ParticipantId, &ProducerId) + Send + Sync + 'static>(
        &self,
        callback: F,
    ) -> HandlerId {
        self.inner.handlers.producer_remove.add(Arc::new(callback))
    }

    /// Subscribe to notification when room is closed.
    pub fn on_close<F: FnOnce() + Send + 'static>(&self, callback: F) -> HandlerId {
        self.inner.handlers.close.add(Box::new(callback))
    }

    pub fn downgrade(&self) -> WeakRoom {
        WeakRoom {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

/// Similar to `Room`, but doesn't prevent room from being destroyed.
#[derive(Debug, Clone)]
pub struct WeakRoom {
    inner: Weak<Inner>,
}

impl WeakRoom {
    pub fn upgrade(&self) -> Option<Room> {
        // Upgrade `WeakRoom` to `Room`, may return `None` if underlying room was destroyed already.
        self.inner.upgrade().map(|inner| Room { inner })
    }
}

/// List of codecs that SFU will accept from clients
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
