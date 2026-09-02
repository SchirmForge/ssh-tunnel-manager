// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Typed bridge between GLib's main context and the Tokio runtime.

use std::io;

use ssh_tunnel_gui_core::{
    AppRuntime, ControllerEffect, ControllerEvent, DaemonClientConfig, RuntimeResult,
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum RuntimeMessage {
    EffectFinished(RuntimeResult),
    Event(ControllerEvent),
    InitializationFailed(String),
    ListenerFailed(String),
}

#[derive(Clone)]
pub struct RuntimeBridge {
    effect_sender: mpsc::UnboundedSender<ControllerEffect>,
}

impl RuntimeBridge {
    pub fn spawn_with_config(
        config: DaemonClientConfig,
    ) -> io::Result<(Self, mpsc::UnboundedReceiver<RuntimeMessage>)> {
        let (effect_sender, mut effect_receiver) = mpsc::unbounded_channel();
        let (message_sender, message_receiver) = mpsc::unbounded_channel();

        std::thread::Builder::new()
            .name("gui-v2-runtime".to_string())
            .spawn(move || {
                let tokio_runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("gui-v2-worker")
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = message_sender
                            .send(RuntimeMessage::InitializationFailed(error.to_string()));
                        return;
                    }
                };

                tokio_runtime.block_on(async move {
                    let app_runtime = match AppRuntime::with_config(config) {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = message_sender
                                .send(RuntimeMessage::InitializationFailed(error.to_string()));
                            return;
                        }
                    };

                    // Establish the complete structured snapshot before SSE can
                    // deliver tunnel events. This prevents an early status code
                    // from being applied before its profile inventory exists.
                    let initial = app_runtime.execute(ControllerEffect::Refresh).await;
                    if message_sender
                        .send(RuntimeMessage::EffectFinished(initial))
                        .is_err()
                    {
                        return;
                    }

                    let listener_runtime = app_runtime.clone();
                    let listener_sender = message_sender.clone();
                    tokio::spawn(async move {
                        match listener_runtime.listen().await {
                            Ok(mut events) => {
                                while let Some(event) = events.recv().await {
                                    if listener_sender.send(RuntimeMessage::Event(event)).is_err() {
                                        return;
                                    }
                                }
                            }
                            Err(error) => {
                                let _ = listener_sender
                                    .send(RuntimeMessage::ListenerFailed(error.to_string()));
                            }
                        }
                    });

                    while let Some(effect) = effect_receiver.recv().await {
                        let result = app_runtime.execute(effect).await;
                        if message_sender
                            .send(RuntimeMessage::EffectFinished(result))
                            .is_err()
                        {
                            return;
                        }
                    }
                });
            })?;

        Ok((Self { effect_sender }, message_receiver))
    }

    pub fn execute(
        &self,
        effect: ControllerEffect,
    ) -> Result<(), mpsc::error::SendError<ControllerEffect>> {
        self.effect_sender.send(effect)
    }
}
