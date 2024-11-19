use futures::future::Future;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use log::{debug, error, info};
use serde_json::Value;
use sha2::Sha256;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::subscription;
use crate::{message, SubscribeResult};

type HmacSha256 = Hmac<Sha256>;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Cannot join to a task")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Tungstenite error")]
    TungsteniteError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Tungstenite error")]
    TungsteniteErrorString(String),

    #[error("Error \"{}\" ({code}) when subscribing to {} (msgid:{id})", message.as_ref().unwrap_or(&"unknown".to_owned()), channel.as_ref().unwrap_or(&"unknown".to_owned()))]
    SubscriptionError {
        id: i64,
        code: u64,
        message: Option<String>,
        channel: Option<String>,
    },

    #[error("Serde error")]
    SerdeError(#[from] serde_json::error::Error),

    #[error("Server closed de communication")]
    CloseError { frame: Option<CloseFrame<'static>> },

    #[error("Unexpected message")]
    UnexpectedMessageError { message: Message },

    #[error("Not connected")]
    NotConnectedError,

    #[error("Invalid sha length")]
    ShaInvalidLength(#[from] hmac::digest::InvalidLength),
}

type EventType<T, Fut> =
    Arc<Mutex<dyn Fn(Result<message::SubscribeResult, CryptoError>, T) -> Fut + Send + Sync>>;
type WriterType =
    Option<Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>>;

pub struct CryptoClient<Fut: Future<Output = ()> + Send + Sync + 'static, T> {
    //events: Arc<Mutex<dyn Fn(Result<message::SubscribeResult>, std::sync::Arc<flume::Sender<T>>)-> Fut + Send + Sync>>,
    events: EventType<T, Fut>,
    reader_join: Option<JoinHandle<Result<(), CryptoError>>>,
    writer: WriterType,
    message_id: u64,
    //sender: std::sync::Arc<flume::Sender<T>>
    container: T,
    market_url: String,
    user_url: String,
}

fn nonce() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n.as_millis(),
        Err(_) => 0,
    }
}

impl<Fut: Future<Output = ()> + Send + Sync + 'static, T: Send + 'static> CryptoClient<Fut, T>
where
    T: Clone,
{
    //pub fn new(f: impl Fn(Result<message::SubscribeResult>, std::sync::Arc<flume::Sender<T>>)->Fut + Send + Sync + 'static, sender: std::sync::Arc<flume::Sender<T>>) -> CryptoTransport<Fut, T> {
    pub fn new(
        f: impl Fn(Result<message::SubscribeResult, CryptoError>, T) -> Fut + Send + Sync + 'static,
        container: T,
    ) -> CryptoClient<Fut, T> {
        CryptoClient {
            events: Arc::new(Mutex::new(f)),
            reader_join: None,
            writer: None,
            message_id: 1,
            container,
            market_url: "wss://stream.crypto.com/v2/market".to_string(),
            user_url: "wss://stream.crypto.com/v2/user".to_string(),
        }
    }

    pub fn with_market_url(mut self, url: String) -> Self {
        self.market_url = url;
        self
    }

    pub fn with_user_url(mut self, url: String) -> Self {
        self.user_url = url;
        self
    }

    pub async fn wait(&mut self) -> Result<(), CryptoError> {
        if let Some(join) = self.reader_join.as_mut() {
            if join.is_finished() {
                Ok(())
            } else {
                join.await?
            }
        } else {
            Ok(())
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), CryptoError> {
        info!("Disconnecting");
        if let Some(writer) = self.writer.as_mut() {
            debug!("Closing connection");
            writer.lock().await.close().await?;
            debug!("Connection closed");
        }

        if let Some(reader) = self.reader_join.as_mut() {
            debug!("Closing reader");
            reader.abort();
            reader.await.ok();
            debug!("Reader closed");
        }
        info!("Disconnected");
        Ok(())
    }

    pub async fn connect_market(&mut self) -> Result<(), CryptoError> {
        let market_url = self.market_url.clone();
        self.connect(&market_url).await?;
        Ok(())
    }

    pub async fn connect_user(&mut self) -> Result<(), CryptoError> {
        let user_url = self.user_url.clone();

        self.connect(&user_url).await?;
        Ok(())
    }

    pub async fn connect(&mut self, uri: &str) -> Result<(), CryptoError> {
        info!("Connecting");
        let connection = connect_async(uri).await?;
        let (ws_stream, _) = connection;

        let (write, mut read) = ws_stream.split();
        let writer = Arc::new(Mutex::new(write));
        let inner_writer = writer.clone();

        let events = Arc::clone(&self.events);

        //let cosa = self.sender.clone();
        let cosa = self.container.clone();
        let join = tokio::spawn(async move {
            let top_inner_cosa = cosa.clone();
            let mut join_result: Result<(), CryptoError> = Ok(());

            info!("Listener ready");
            while let Some(next) = read.next().await {
                let inner_cosa = top_inner_cosa.clone();
                match next {
                    Ok(message) => {
                        let e = events.lock().await;
                        match message {
                            Message::Text(text) => {
                                debug!("Text received {text}");
                                // Json parse
                                match serde_json::from_str::<message::Message>(&text) {
                                    Ok(msg) => match msg {
                                        message::Message::HeartbeatRequest { id } => {
                                            debug!("heartbeat received");
                                            let message =
                                                subscription::Request::HeartbeatResponse { id };
                                            match serde_json::to_string(&message) {
                                                Ok(text) => {
                                                    if let Err(error) = inner_writer
                                                        .lock()
                                                        .await
                                                        .send(Message::text(text))
                                                        .await
                                                    {
                                                        error!("Cannot send heartbeat");
                                                        e(
                                                            Err(CryptoError::TungsteniteError(
                                                                error,
                                                            )),
                                                            inner_cosa,
                                                        );
                                                    } else {
                                                        debug!("heartbeat sent");
                                                    }
                                                }
                                                Err(error) => {
                                                    error!("Cannot serialize heartbeat");
                                                    e(
                                                        Err(CryptoError::SerdeError(error)),
                                                        inner_cosa,
                                                    );
                                                }
                                            }
                                        }
                                        message::Message::SubscriptionResponse {
                                            result,
                                            id,
                                            code,
                                            channel,
                                            message,
                                        } => {
                                            if let Some(result) = result {
                                                debug!("Message received: {:?}", result);
                                                e(Ok(result), inner_cosa).await;
                                            } else if code != 0 {
                                                e(
                                                    Err(CryptoError::SubscriptionError {
                                                        id,
                                                        code,
                                                        message,
                                                        channel,
                                                    }),
                                                    inner_cosa,
                                                );
                                            }
                                        }
                                        message::Message::UnsubscriptionResponse { id, code } => {
                                            debug!("Unsubscription: {id} {code}");
                                            e(
                                                Ok(SubscribeResult::UnsubscriptionResult {
                                                    success: code == 0,
                                                }),
                                                inner_cosa,
                                            )
                                            .await;
                                        }
                                        message::Message::AuthResponse { id, code } => {
                                            debug!("Notify auth response: {id} {code}");
                                            e(
                                                Ok(SubscribeResult::AuthResult {
                                                    success: code == 0,
                                                }),
                                                inner_cosa,
                                            )
                                            .await;
                                        }
                                    },
                                    Err(err) => {
                                        error!("Error when parsing JSON:\n{}\n{}", text, err);
                                        e(Err(CryptoError::SerdeError(err)), inner_cosa).await;
                                    }
                                }
                            }
                            Message::Ping(message) => {
                                debug!("Ping received {:?}", message);
                                if let Err(error) =
                                    inner_writer.lock().await.send(Message::Pong(message)).await
                                {
                                    error!("Cannot send pong");
                                    e(Err(CryptoError::TungsteniteError(error)), inner_cosa).await;
                                } else {
                                    debug!("Pong sent");
                                }
                            }
                            Message::Pong(message) => {
                                debug!("PONG RECEIVED {:?}", message);
                            }
                            Message::Close(frame) => {
                                e(
                                    Err(CryptoError::CloseError {
                                        frame: frame.clone(),
                                    }),
                                    inner_cosa,
                                )
                                .await;
                                return Err(CryptoError::CloseError { frame });
                            }
                            message => {
                                error!("Unexpected message {:?}", message);
                                e(
                                    Err(CryptoError::UnexpectedMessageError { message }),
                                    inner_cosa,
                                )
                                .await;
                            }
                        }
                    }
                    Err(error) => {
                        let e = events.lock().await;
                        error!("Websocket read error: {:?}", error);
                        e(
                            Err(CryptoError::TungsteniteErrorString(error.to_string())),
                            inner_cosa,
                        )
                        .await;
                        join_result = Err(CryptoError::TungsteniteError(error));
                    }
                }
            }
            join_result
        });

        self.reader_join = Some(join);
        self.writer = Some(writer);
        info!("Connected");
        Ok(())
    }

    pub async fn subscribe(&mut self, param: Value) -> Result<(), CryptoError> {
        debug!("Subscribing to {:?} param", param);
        if let Some(writer) = self.writer.as_mut() {
            let message = subscription::Request::Subscribe {
                id: self.message_id,
                params: param,
                nonce: nonce(),
            };

            let text = serde_json::to_string(&message)?;
            writer.lock().await.send(Message::text(text)).await?;
            // Increase message_id only if the message was actually sent
            self.message_id += 1;
            debug!("New message id {:?}", self.message_id);
            Ok(())
        } else {
            Err(CryptoError::NotConnectedError)
        }
    }

    pub async fn unsubscribe(&mut self, channels: Vec<String>) -> Result<(), CryptoError> {
        debug!("Unsubscribing to {:?} channels", channels.len());
        if let Some(writer) = self.writer.as_mut() {
            let message = subscription::Request::Unsubscribe {
                id: self.message_id,
                params: subscription::UnsubscribeParams { channels },
                nonce: nonce(),
            };

            let text = serde_json::to_string(&message)?;
            writer.lock().await.send(Message::text(text)).await?;
            // Increase message_id only if the message was actually sent
            self.message_id += 1;
            debug!("New message id {:?}", self.message_id);
            Ok(())
        } else {
            Err(CryptoError::NotConnectedError)
        }
    }

    pub async fn auth(&mut self, api_key: &str, api_secret: &str) -> Result<(), CryptoError> {
        if let Some(writer) = self.writer.as_mut() {
            let n = nonce();
            let message_to_sig = [
                "public/auth".into(),
                self.message_id.to_string(),
                api_key.to_owned(),
                n.to_string(),
            ]
            .concat();
            let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())?;
            mac.update(message_to_sig.as_bytes());
            let result = mac.finalize();
            let f = result.into_bytes();

            let message = subscription::Request::Auth {
                id: self.message_id,
                api_key: api_key.to_owned(),
                sig: hex::encode(f),
                nonce: n,
            };

            let text = serde_json::to_string(&message)?;
            writer.lock().await.send(Message::text(text)).await?;
            // Increase message_id only if the message was actually sent
            self.message_id += 1;
            Ok(())
        } else {
            Err(CryptoError::NotConnectedError)
        }
    }
}
