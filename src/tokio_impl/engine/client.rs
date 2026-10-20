// Copyright (c) 2022-2026, Radu Racariu.

//! [`EngineClient`] — the typed handle Rust callers drive a running
//! engine through.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use libhaystack::val::Value;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::base::engine::messages::{
    BlockDefinition, ClientId, EngineRequest, EngineResponse, Envelope, RequestId, RequestSender,
    ResponseMessage, WatchId, WatchMessage,
};
use crate::base::error::{EngineError, RequestError};
use crate::base::program::{EditorMetadata, LinkData, Position, Program};

/// A typed handle to an engine: an async method per
/// [`EngineRequest`], each sending its request and awaiting the
/// matching response.
///
/// Create one from an idle engine with
/// [`Engine::client`](crate::base::engine::Engine::client), before
/// [`run`](crate::base::engine::Engine::run) consumes it. Clones share
/// the client's channels, so a clone is how a running engine gets more
/// handles — one per task, or one shared by many. On native targets a
/// client is [`Send`] + [`Sync`], so it can drive the engine from
/// another task or thread.
///
/// Responses are correlated by [`RequestId`]: any number of requests —
/// from one clone or many — can be in flight at once, and each method
/// returns its own request's answer. Dropping a method's future before
/// it completes abandons the request, not the client: the engine still
/// handles it, and its response is discarded when it arrives.
///
/// Requests sent to an idle engine wait until it runs. Once the engine
/// is dropped every method fails with
/// [`EngineError::EngineGone`](crate::EngineError::EngineGone).
///
/// # Examples
///
/// ```
/// use logic_mesh::{base::engine::Engine, single_threaded::SingleThreadedEngine};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut engine = SingleThreadedEngine::new();
/// let client = engine.client();
///
/// let driver = async move {
///     let id = client.add_block("Add", None, None).await?;
///     client.write_input(id, "in0", 2.into()).await?;
///     let block = client.inspect_block(id).await?;
///     assert_eq!(block.name, "Add");
///     client.shutdown().await?;
///     Ok::<_, logic_mesh::RequestError>(())
/// };
/// let (engine, driven) = tokio::join!(engine.run(), driver);
/// driven?;
/// assert_eq!(engine.block_handles().len(), 1);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct EngineClient {
    inner: Arc<Inner>,
}

/// What the clones of a client share.
struct Inner {
    /// The id the engine routes this client's responses by.
    client: ClientId,
    /// The engine's request channel.
    requests: RequestSender,
    /// The id the next request gets.
    next_id: AtomicU64,
    /// Where to deliver the response to each request in flight.
    pending: Mutex<HashMap<RequestId, oneshot::Sender<EngineResponse>>>,
    /// The engine's responses to this client. Whichever request holds
    /// the lock reads it, delivering every response it reads through
    /// [`pending`](Self::pending), until its own arrives.
    responses: tokio::sync::Mutex<mpsc::UnboundedReceiver<ResponseMessage>>,
}

impl Inner {
    /// The requests in flight. A panic while the lock was held cannot
    /// leave the map inconsistent, so a poisoned lock is used as is.
    fn pending(&self) -> MutexGuard<'_, HashMap<RequestId, oneshot::Sender<EngineResponse>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Delivers `message` to the request awaiting it, if any still does.
    fn deliver(&self, message: ResponseMessage) {
        let Envelope { id, body, .. } = message;
        match self.pending().remove(&id) {
            // The request may have been abandoned since; then the
            // response goes with its receiver.
            Some(waiting) => {
                let _ = waiting.send(body);
            }
            None => log::debug!(
                "Discarding the {} response to {id} of {}: nothing awaits it",
                body.name(),
                self.client
            ),
        }
    }
}

/// Removes a request from [`Inner::pending`] when its future ends,
/// answered or abandoned.
struct InFlight<'a> {
    inner: &'a Inner,
    id: RequestId,
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.inner.pending().remove(&self.id);
    }
}

/// A watch [`EngineClient::watch`] opened: its id, and the block change
/// notifications it delivers.
///
/// The notifications end once the watch is closed with
/// [`EngineClient::unwatch`] or the engine is dropped. Dropping the
/// receiver closes the watch too, the next time the engine notifies it.
#[derive(Debug)]
#[non_exhaustive]
pub struct Watch {
    /// The watch's id, to close it with.
    pub id: WatchId,
    /// The block change notifications.
    pub notifications: mpsc::UnboundedReceiver<WatchMessage>,
}

/// Unwraps the `result` of the [`EngineResponse`] variant answering a
/// `$variant` request, or reports another variant as unexpected.
macro_rules! expect_response {
    ($response:expr, $variant:ident) => {
        match $response {
            EngineResponse::$variant { result } => result,
            other => Err(EngineError::UnexpectedResponse {
                request: stringify!($variant),
                response: other.name(),
            }
            .into()),
        }
    };
}

impl EngineClient {
    /// A client that sends its requests on `requests`, stamped with
    /// `client`, and reads the engine's responses to them from
    /// `responses` — the two ends of
    /// [`Engine::create_message_channel`](crate::base::engine::Engine::create_message_channel),
    /// which [`Engine::client`](crate::base::engine::Engine::client)
    /// wires up already.
    pub fn new(
        client: ClientId,
        requests: RequestSender,
        responses: mpsc::UnboundedReceiver<ResponseMessage>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                client,
                requests,
                next_id: AtomicU64::new(0),
                pending: Mutex::default(),
                responses: tokio::sync::Mutex::new(responses),
            }),
        }
    }

    /// The id the engine knows this client, and its clones, by.
    pub fn id(&self) -> ClientId {
        self.inner.client
    }

    /// Sends `request` and returns the engine's response to it, whatever
    /// its kind. The typed methods are built on this.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::EngineGone`](crate::EngineError::EngineGone)
    /// if the engine is dropped before it answers. A request the engine
    /// handled but failed is an `Ok` response carrying the error.
    pub async fn request(&self, request: EngineRequest) -> Result<EngineResponse, RequestError> {
        let inner = &*self.inner;
        let id = RequestId(inner.next_id.fetch_add(1, Ordering::Relaxed));
        let (waiting, mut response) = oneshot::channel();
        inner.pending().insert(id, waiting);
        let _in_flight = InFlight { inner, id };

        inner
            .requests
            .send(Envelope::new(inner.client, id, request))
            .await
            .map_err(|_| EngineError::EngineGone)?;

        tokio::select! {
            biased;
            // Another request read the response off the channel.
            answered = &mut response => answered.map_err(|_| EngineError::EngineGone.into()),
            // Read the channel for every request in flight until this
            // one's response arrives.
            mut responses = inner.responses.lock() => loop {
                match response.try_recv() {
                    Ok(answer) => return Ok(answer),
                    Err(oneshot::error::TryRecvError::Closed) => {
                        return Err(EngineError::EngineGone.into());
                    }
                    Err(oneshot::error::TryRecvError::Empty) => {}
                }
                let Some(message) = responses.recv().await else {
                    return Err(EngineError::EngineGone.into());
                };
                inner.deliver(message);
            },
        }
    }

    /// Adds a block of type `name`, under `block_id` if given, and
    /// returns its id. Without `library`, every library is searched. See
    /// [`EngineRequest::AddBlock`].
    pub async fn add_block(
        &self,
        name: &str,
        block_id: Option<Uuid>,
        library: Option<&str>,
    ) -> Result<Uuid, RequestError> {
        let request = EngineRequest::AddBlock {
            name: name.to_string(),
            block_id,
            library: library.map(str::to_string),
        };
        expect_response!(self.request(request).await?, AddBlock)
    }

    /// Removes block `block` with all its links, returning its id. See
    /// [`EngineRequest::RemoveBlock`].
    pub async fn remove_block(&self, block: Uuid) -> Result<Uuid, RequestError> {
        let request = EngineRequest::RemoveBlock { block };
        expect_response!(self.request(request).await?, RemoveBlock)
    }

    /// Links two block pins, returning the link with its id. See
    /// [`EngineRequest::Connect`].
    pub async fn connect(&self, link: LinkData) -> Result<LinkData, RequestError> {
        let request = EngineRequest::Connect { link };
        expect_response!(self.request(request).await?, Connect)
    }

    /// Removes link `link`, returning whether it existed. See
    /// [`EngineRequest::Disconnect`].
    pub async fn disconnect(&self, link: Uuid) -> Result<bool, RequestError> {
        let request = EngineRequest::Disconnect { link };
        expect_response!(self.request(request).await?, Disconnect)
    }

    /// Writes `value` to input `pin` of block `block`, returning the
    /// input's previous value. See [`EngineRequest::WriteInput`].
    pub async fn write_input(
        &self,
        block: Uuid,
        pin: &str,
        value: Value,
    ) -> Result<Option<Value>, RequestError> {
        let request = EngineRequest::WriteInput {
            block,
            pin: pin.to_string(),
            value,
        };
        expect_response!(self.request(request).await?, WriteInput)
    }

    /// Writes `value` to output `pin` of block `block`, returning the
    /// output's previous value. See [`EngineRequest::WriteOutput`].
    pub async fn write_output(
        &self,
        block: Uuid,
        pin: &str,
        value: Value,
    ) -> Result<Value, RequestError> {
        let request = EngineRequest::WriteOutput {
            block,
            pin: pin.to_string(),
            value,
        };
        expect_response!(self.request(request).await?, WriteOutput)
    }

    /// Returns block `block`'s state and pin values. See
    /// [`EngineRequest::InspectBlock`].
    pub async fn inspect_block(&self, block: Uuid) -> Result<BlockDefinition, RequestError> {
        let request = EngineRequest::InspectBlock { block };
        expect_response!(self.request(request).await?, InspectBlock)
    }

    /// Evaluates block type `name` once with `inputs`, returning its
    /// output values. See [`EngineRequest::EvalBlock`], which also says
    /// how a panicking or never-finishing evaluation fails.
    pub async fn eval_block(
        &self,
        name: &str,
        inputs: Vec<Value>,
        library: Option<&str>,
    ) -> Result<Vec<Value>, RequestError> {
        let request = EngineRequest::EvalBlock {
            name: name.to_string(),
            inputs,
            library: library.map(str::to_string),
        };
        expect_response!(self.request(request).await?, EvalBlock)
    }

    /// Returns the engine's current program in save format. See
    /// [`EngineRequest::SaveProgram`].
    pub async fn save_program(&self) -> Result<Program, RequestError> {
        expect_response!(self.request(EngineRequest::SaveProgram).await?, SaveProgram)
    }

    /// Loads `program`, all or nothing. See
    /// [`EngineRequest::LoadProgram`].
    pub async fn load_program(&self, program: Program) -> Result<(), RequestError> {
        let request = EngineRequest::LoadProgram { program };
        expect_response!(self.request(request).await?, LoadProgram)
    }

    /// Updates the label and position block `block` is saved with. See
    /// [`EngineRequest::SetBlockMetadata`].
    pub async fn set_block_metadata(
        &self,
        block: Uuid,
        label: Option<String>,
        position: Option<Position>,
    ) -> Result<(), RequestError> {
        let request = EngineRequest::SetBlockMetadata {
            block,
            label,
            position,
        };
        expect_response!(self.request(request).await?, SetBlockMetadata)
    }

    /// Replaces, or with `None` removes, the editor metadata block
    /// `block` is saved with. See [`EngineRequest::SetBlockEditor`].
    pub async fn set_block_editor(
        &self,
        block: Uuid,
        editor: Option<EditorMetadata>,
    ) -> Result<(), RequestError> {
        let request = EngineRequest::SetBlockEditor { block, editor };
        expect_response!(self.request(request).await?, SetBlockEditor)
    }

    /// Opens a watch on every block's changes. See
    /// [`EngineRequest::Watch`].
    pub async fn watch(&self) -> Result<Watch, RequestError> {
        let (sender, notifications) = mpsc::unbounded_channel();
        let request = EngineRequest::Watch { sender };
        let id = expect_response!(self.request(request).await?, Watch)?;
        Ok(Watch { id, notifications })
    }

    /// Closes watch `watch`, returning whether it was open. See
    /// [`EngineRequest::Unwatch`].
    pub async fn unwatch(&self, watch: WatchId) -> Result<bool, RequestError> {
        let request = EngineRequest::Unwatch { watch };
        expect_response!(self.request(request).await?, Unwatch)
    }

    /// Attaches the registered connector `name`, returning its name. See
    /// [`EngineRequest::AddConnector`].
    pub async fn add_connector(&self, name: &str) -> Result<String, RequestError> {
        let request = EngineRequest::AddConnector {
            name: name.to_string(),
        };
        expect_response!(self.request(request).await?, AddConnector)
    }

    /// Detaches the engine-managed connector `name`, returning its name.
    /// See [`EngineRequest::RemoveConnector`].
    pub async fn remove_connector(&self, name: &str) -> Result<String, RequestError> {
        let request = EngineRequest::RemoveConnector {
            name: name.to_string(),
        };
        expect_response!(self.request(request).await?, RemoveConnector)
    }

    /// Returns the names of the connectors the engine manages, as
    /// [`Engine::managed_connectors`](crate::base::engine::Engine::managed_connectors)
    /// does on an idle engine. See [`EngineRequest::ManagedConnectors`].
    pub async fn managed_connectors(&self) -> Result<Vec<String>, RequestError> {
        expect_response!(
            self.request(EngineRequest::ManagedConnectors).await?,
            ManagedConnectors
        )
    }

    /// Pauses block execution, returning once no block starts executing
    /// any more. See [`EngineRequest::Pause`].
    pub async fn pause(&self) -> Result<(), RequestError> {
        expect_response!(self.request(EngineRequest::Pause).await?, Pause)
    }

    /// Resumes block execution. See [`EngineRequest::Resume`].
    pub async fn resume(&self) -> Result<(), RequestError> {
        expect_response!(self.request(EngineRequest::Resume).await?, Resume)
    }

    /// Resets the engine, returning once its blocks, links and managed
    /// connectors are gone. See [`EngineRequest::Reset`].
    pub async fn reset(&self) -> Result<(), RequestError> {
        expect_response!(self.request(EngineRequest::Reset).await?, Reset)
    }

    /// Shuts the engine down, returning once block execution and the
    /// managed connectors stopped. See [`EngineRequest::Shutdown`].
    pub async fn shutdown(&self) -> Result<(), RequestError> {
        expect_response!(self.request(EngineRequest::Shutdown).await?, Shutdown)
    }
}

/// Shows the client's id and how many of its requests are in flight.
impl fmt::Debug for EngineClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineClient")
            .field("id", &self.inner.client)
            .field("in_flight", &self.inner.pending().len())
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use std::time::Duration;

    use assert_matches::assert_matches;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    use super::EngineClient;
    use crate::base::engine::messages::{
        ClientId, EngineRequest, EngineResponse, Envelope, RequestMessage, ResponseMessage,
    };
    use crate::base::error::{EngineError, Error, ErrorCode};

    /// A client wired to channels the test plays the engine on.
    fn fake_engine() -> (
        EngineClient,
        mpsc::Receiver<RequestMessage>,
        mpsc::UnboundedSender<ResponseMessage>,
    ) {
        let (requests, received) = mpsc::channel(8);
        let (respond, responses) = mpsc::unbounded_channel();
        let client = EngineClient::new(ClientId::new(7), requests, responses);
        (client, received, respond)
    }

    /// The client is meant to be handed to other tasks and threads.
    #[test]
    fn client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<EngineClient>();
    }

    /// Requests in flight together get their own responses, though they
    /// arrive in the reverse order.
    #[tokio::test(flavor = "current_thread")]
    async fn responses_are_matched_by_request_id() {
        let (client, mut received, respond) = fake_engine();

        let names = ["first", "second", "third"];
        let calls = names.map(|name| {
            let client = client.clone();
            tokio::spawn(async move { client.add_connector(name).await })
        });

        let mut requests = Vec::new();
        for _ in names {
            let request = timeout(Duration::from_secs(2), received.recv())
                .await
                .expect("the requests are sent")
                .expect("the client is alive");
            assert_eq!(request.client, client.id());
            requests.push(request);
        }
        for request in requests.into_iter().rev() {
            let EngineRequest::AddConnector { name } = request.body else {
                panic!("Expected AddConnector, got {}", request.body.name());
            };
            let response = EngineResponse::AddConnector { result: Ok(name) };
            respond
                .send(Envelope::new(request.client, request.id, response))
                .expect("the client is alive");
        }

        for (call, name) in calls.into_iter().zip(names) {
            let answer = timeout(Duration::from_secs(2), call)
                .await
                .expect("answered")
                .expect("the call completes");
            assert_eq!(answer, Ok(name.to_string()));
        }
    }

    /// An abandoned request's response is discarded instead of answering
    /// the next request.
    #[tokio::test(flavor = "current_thread")]
    async fn an_abandoned_request_does_not_desynchronize_the_client() {
        let (client, mut received, respond) = fake_engine();

        let abandoned = timeout(Duration::from_millis(50), client.managed_connectors()).await;
        assert!(abandoned.is_err(), "nothing answered it");
        let stale = received.recv().await.expect("the request was sent");
        respond
            .send(Envelope::new(
                stale.client,
                stale.id,
                EngineResponse::ManagedConnectors {
                    result: Ok(vec!["stale".to_string()]),
                },
            ))
            .expect("the client is alive");

        let call = {
            let client = client.clone();
            tokio::spawn(async move { client.managed_connectors().await })
        };
        let fresh = received.recv().await.expect("the request was sent");
        respond
            .send(Envelope::new(
                fresh.client,
                fresh.id,
                EngineResponse::ManagedConnectors {
                    result: Ok(vec!["fresh".to_string()]),
                },
            ))
            .expect("the client is alive");

        let answer = call.await.expect("the call completes");
        assert_eq!(answer, Ok(vec!["fresh".to_string()]));
        assert_eq!(
            format!("{client:?}"),
            "EngineClient { id: ClientId(7), in_flight: 0, .. }"
        );
    }

    /// A response of the wrong kind is an error, not a panic.
    #[tokio::test(flavor = "current_thread")]
    async fn a_response_of_another_kind_is_unexpected() {
        let (client, mut received, respond) = fake_engine();

        let call = {
            let client = client.clone();
            tokio::spawn(async move { client.pause().await })
        };
        let request = received.recv().await.expect("the request was sent");
        respond
            .send(Envelope::new(
                request.client,
                request.id,
                EngineResponse::Resume { result: Ok(()) },
            ))
            .expect("the client is alive");

        let err = call
            .await
            .expect("the call completes")
            .expect_err("a Resume does not answer a Pause");
        assert_eq!(err.code, ErrorCode::UnexpectedResponse);
        assert_matches!(
            err.error(),
            Some(Error::Engine(EngineError::UnexpectedResponse {
                request: "Pause",
                response: "Resume"
            }))
        );
    }

    /// Once the engine's ends of the channels are gone, requests fail
    /// instead of waiting forever.
    #[tokio::test(flavor = "current_thread")]
    async fn a_dropped_engine_fails_requests() {
        let (client, received, respond) = fake_engine();
        drop(respond);

        let call = {
            let client = client.clone();
            tokio::spawn(async move { client.reset().await })
        };
        let err = timeout(Duration::from_secs(2), call)
            .await
            .expect("answered")
            .expect("the call completes")
            .expect_err("nobody answers");
        assert_eq!(err.code, ErrorCode::EngineGone);

        drop(received);
        let err = client.reset().await.expect_err("nobody receives");
        assert_eq!(err.code, ErrorCode::EngineGone);
    }
}
