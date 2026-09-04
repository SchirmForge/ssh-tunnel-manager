// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Toolkit-neutral execution of controller effects and daemon events.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use ssh_tunnel_common::{
    get_password, has_password, is_key_encrypted, load_profile_by_id,
    profile_uses_client_credential, remove_password, store_password, validate_key_passphrase,
    validate_ssh_key_file, AuthRequest, AuthRequestType, AuthType, ConnectionMode,
    DaemonClientConfig, EventListener, PasswordStorage, Profile, TunnelEvent, TunnelStatus, Utc,
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::actions::{profile_changes_require_reconnect, ControllerEffect, ProfileOperation};
use crate::client_setup::{ClientSetupDiscovery, ClientSetupRepository};
use crate::controller::{ControllerEvent, OperationOutcome, TunnelRuntimeSnapshot};
use crate::daemon::DaemonClient;
use crate::editor::{
    CredentialUpdate, ProfileDeletionRequest, ProfileEditorDraft, ProfileEditorMode,
    ProfileEditorSession, ProfileReconnectRequest, ProfileSaveRequest, SecretValue,
    StoredCredentialState,
};
use crate::preferences::UiPreferencesRepository;
use crate::profiles::{delete_profile, load_profiles, save_profile, validate_profile};

/// A request that must be handled by the presentation adapter itself.
#[derive(Debug, Clone)]
pub enum PresentationRequest {
    ProfileEditor(Box<ProfileEditorSession>),
    ConfirmDelete(ProfileDeletionRequest),
    ConfirmReconnect(ProfileReconnectRequest),
}

/// Result of executing one controller effect.
#[derive(Debug, Default)]
pub struct RuntimeResult {
    pub events: Vec<ControllerEvent>,
    pub presentation: Option<PresentationRequest>,
}

impl RuntimeResult {
    fn event(event: ControllerEvent) -> Self {
        Self {
            events: vec![event],
            presentation: None,
        }
    }

    fn presentation(request: PresentationRequest) -> Self {
        Self {
            events: Vec::new(),
            presentation: Some(request),
        }
    }
}

/// Shared application runtime used by GTK now and a future tray adapter later.
///
/// Callers run [`Self::execute`] away from their UI thread, apply every returned
/// [`ControllerEvent`] to the controller in order, and handle an optional
/// [`PresentationRequest`] on the toolkit's main context.
#[derive(Clone)]
pub struct AppRuntime {
    daemon_client: DaemonClient,
    preferences: UiPreferencesRepository,
    client_credentials_offered: Arc<Mutex<HashSet<Uuid>>>,
}

impl AppRuntime {
    pub fn for_default_config() -> Result<Self> {
        match ClientSetupRepository::for_default_paths()?.discover() {
            ClientSetupDiscovery::Ready(config) => Self::with_config(config),
            ClientSetupDiscovery::SnippetAvailable | ClientSetupDiscovery::SetupRequired(_) => {
                anyhow::bail!("client connection setup is required before runtime startup")
            }
        }
    }

    pub fn with_config(config: DaemonClientConfig) -> Result<Self> {
        Ok(Self {
            daemon_client: DaemonClient::with_config(config)?,
            preferences: UiPreferencesRepository::for_default_path()?,
            client_credentials_offered: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn with_parts(daemon_client: DaemonClient, preferences: UiPreferencesRepository) -> Self {
        Self {
            daemon_client,
            preferences,
            client_credentials_offered: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn daemon_client(&self) -> &DaemonClient {
        &self.daemon_client
    }

    /// Execute one effect without referring to any GUI toolkit type.
    pub async fn execute(&self, effect: ControllerEffect) -> RuntimeResult {
        match effect {
            ControllerEffect::Refresh => self.refresh().await,
            ControllerEffect::ShutdownDaemon => self.shutdown_daemon().await,
            ControllerEffect::OpenCreateProfile => RuntimeResult::presentation(
                PresentationRequest::ProfileEditor(Box::new(ProfileEditorSession {
                    mode: ProfileEditorMode::Create,
                    draft: ProfileEditorDraft::new(self.daemon_is_local()),
                    client_credential: StoredCredentialState::NotStored,
                    daemon_is_local: self.daemon_is_local(),
                })),
            ),
            ControllerEffect::OpenEditProfile(profile_id) => {
                self.open_profile_editor(profile_id, ProfileEditorMode::Edit)
                    .await
            }
            ControllerEffect::OpenDuplicateProfile(profile_id) => {
                self.open_profile_editor(profile_id, ProfileEditorMode::Duplicate)
                    .await
            }
            ControllerEffect::ConfirmDeleteProfile(profile_id) => {
                self.confirm_delete(profile_id).await
            }
            ControllerEffect::SaveProfile {
                request,
                offer_reconnect,
            } => self.save_profile(*request, offer_reconnect).await,
            ControllerEffect::DeleteProfile(profile_id) => self.delete_profile(profile_id).await,
            ControllerEffect::ConnectProfile(profile_id) => {
                self.start_profile(profile_id, ProfileOperation::Connect)
                    .await
            }
            ControllerEffect::CancelConnection(profile_id) => {
                self.stop_profile(profile_id, ProfileOperation::CancelConnection)
                    .await
            }
            ControllerEffect::DisconnectProfile(profile_id) => {
                self.stop_profile(profile_id, ProfileOperation::Disconnect)
                    .await
            }
            ControllerEffect::ReconnectProfile(profile_id) => {
                self.reconnect_profile(profile_id).await
            }
            ControllerEffect::RetryProfile(profile_id) => {
                self.start_profile(profile_id, ProfileOperation::Retry)
                    .await
            }
            ControllerEffect::SetAutoReconnect {
                profile_id,
                enabled,
            } => self.set_auto_reconnect(profile_id, enabled),
            ControllerEffect::PersistPreferences(preferences) => {
                match self.preferences.save(&preferences) {
                    Ok(()) => RuntimeResult::default(),
                    Err(error) => RuntimeResult::event(ControllerEvent::RuntimeError {
                        profile_id: None,
                        message: error.to_string(),
                    }),
                }
            }
            ControllerEffect::SubmitAuthentication(submission) => {
                let result = self
                    .daemon_client
                    .submit_auth_with_id(
                        submission.tunnel_id,
                        submission.request_id,
                        submission.response,
                    )
                    .await;
                match result {
                    Ok(()) => RuntimeResult::default(),
                    Err(error) => {
                        RuntimeResult::event(ControllerEvent::AuthenticationSubmissionFailed {
                            request_id: submission.request_id,
                            message: error.to_string(),
                        })
                    }
                }
            }
            ControllerEffect::CancelAuthentication {
                request_id,
                tunnel_id,
            } => match self.daemon_client.stop_tunnel(tunnel_id).await {
                Ok(()) => RuntimeResult::default(),
                Err(error) => {
                    RuntimeResult::event(ControllerEvent::AuthenticationSubmissionFailed {
                        request_id,
                        message: error.to_string(),
                    })
                }
            },
        }
    }

    /// Subscribe to the daemon's SSE stream and receive controller events.
    ///
    /// Heartbeats prove that the daemon is online. Offline detection remains a
    /// health-refresh responsibility because the common listener reconnects
    /// internally and intentionally hides transient transport failures.
    pub async fn listen(&self) -> Result<mpsc::Receiver<ControllerEvent>> {
        let listener = EventListener::new(self.daemon_client.config.clone());
        let mut daemon_events = listener.listen().await?;
        let (event_tx, event_rx) = mpsc::channel(100);
        let runtime = self.clone();

        tokio::spawn(async move {
            while let Some(event) = daemon_events.recv().await {
                if let TunnelEvent::Starting { id } = &event {
                    runtime.clear_client_credential_offer(*id);
                }

                if let TunnelEvent::AuthRequired { id, request } = &event {
                    if *id == request.tunnel_id {
                        match runtime.answer_client_credential(request).await {
                            ClientCredentialAnswer::Submitted => {
                                if event_tx
                                    .send(ControllerEvent::TunnelStatusChanged {
                                        profile_id: *id,
                                        status: TunnelStatus::WaitingForAuth,
                                    })
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                                continue;
                            }
                            ClientCredentialAnswer::SubmissionFailed(message) => {
                                for controller_event in map_sse_event(event.clone()) {
                                    if event_tx.send(controller_event).await.is_err() {
                                        return;
                                    }
                                }
                                if event_tx
                                    .send(ControllerEvent::RuntimeError {
                                        profile_id: Some(*id),
                                        message,
                                    })
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                                continue;
                            }
                            ClientCredentialAnswer::NotApplicable => {}
                        }
                    }
                }

                for controller_event in map_sse_event(event) {
                    if event_tx.send(controller_event).await.is_err() {
                        return;
                    }
                }
            }
        });

        Ok(event_rx)
    }

    async fn refresh(&self) -> RuntimeResult {
        let mut events = vec![ControllerEvent::DaemonConnectionMode(
            self.daemon_client.config.connection_mode.clone(),
        )];

        match load_profiles() {
            Ok(profiles) => events.push(ControllerEvent::ProfilesLoaded(profiles)),
            Err(error) => events.push(ControllerEvent::RuntimeError {
                profile_id: None,
                message: error.to_string(),
            }),
        }

        match self.preferences.load() {
            Ok(load) => {
                events.push(ControllerEvent::PreferencesLoaded(load.preferences));
                if let Some(warning) = load.warning {
                    events.push(ControllerEvent::RuntimeError {
                        profile_id: None,
                        message: warning,
                    });
                }
            }
            Err(error) => events.push(ControllerEvent::RuntimeError {
                profile_id: None,
                message: error.to_string(),
            }),
        }

        match self.daemon_client.health_check().await {
            Ok(true) => {
                events.push(ControllerEvent::DaemonConnectionChanged(true));

                match self.daemon_client.list_tunnels().await {
                    Ok(tunnels) => events.push(ControllerEvent::TunnelInventory(
                        tunnels
                            .into_iter()
                            .map(|tunnel| TunnelRuntimeSnapshot {
                                profile_id: tunnel.id,
                                status: tunnel.status,
                                pending_auth: tunnel.pending_auth,
                            })
                            .collect(),
                    )),
                    Err(error) => events.push(ControllerEvent::RuntimeError {
                        profile_id: None,
                        message: error.to_string(),
                    }),
                }

                match self.daemon_client.get_daemon_info().await {
                    Ok(info) => {
                        events.push(ControllerEvent::DaemonInfoUpdated(Some(Box::new(info))))
                    }
                    Err(error) => events.push(ControllerEvent::RuntimeError {
                        profile_id: None,
                        message: error.to_string(),
                    }),
                }
            }
            Ok(false) => {
                events.push(ControllerEvent::DaemonConnectionChanged(false));
                events.push(ControllerEvent::DaemonInfoUpdated(None));
            }
            Err(error) => {
                events.push(ControllerEvent::DaemonConnectionChanged(false));
                events.push(ControllerEvent::DaemonInfoUpdated(None));
                events.push(ControllerEvent::RuntimeError {
                    profile_id: None,
                    message: error.to_string(),
                });
            }
        }

        // Always release the controller's refresh state, including offline and
        // partial-error outcomes. Reachability is carried by its own structured
        // event and error text remains display-only.
        events.push(ControllerEvent::RefreshFinished);

        RuntimeResult {
            events,
            presentation: None,
        }
    }

    async fn shutdown_daemon(&self) -> RuntimeResult {
        match self.daemon_client.shutdown_daemon().await {
            Ok(()) => RuntimeResult {
                events: vec![
                    ControllerEvent::DaemonInfoUpdated(None),
                    ControllerEvent::DaemonConnectionChanged(false),
                ],
                presentation: None,
            },
            Err(error) => RuntimeResult::event(ControllerEvent::RuntimeError {
                profile_id: None,
                message: error.to_string(),
            }),
        }
    }

    async fn open_profile_editor(
        &self,
        profile_id: Uuid,
        mode: ProfileEditorMode,
    ) -> RuntimeResult {
        let daemon_is_local = self.daemon_is_local();
        let result = tokio::task::spawn_blocking(move || -> Result<ProfileEditorSession> {
            let profile = load_profile_by_id(&profile_id)
                .with_context(|| format!("Failed to load profile {profile_id}"))?;
            let (draft, client_credential) = match mode {
                ProfileEditorMode::Edit => {
                    let resolved_storage = profile
                        .connection
                        .password_storage
                        .resolved(daemon_is_local);
                    let state = if resolved_storage == PasswordStorage::Client {
                        client_credential_state(profile_id)
                    } else {
                        StoredCredentialState::NotStored
                    };
                    (ProfileEditorDraft::edit(profile, daemon_is_local), state)
                }
                ProfileEditorMode::Duplicate => {
                    let profiles = load_profiles()?;
                    let name = duplicate_name(&profile.metadata.name, &profiles);
                    (
                        ProfileEditorDraft::duplicate(&profile, name, daemon_is_local),
                        StoredCredentialState::NotStored,
                    )
                }
                ProfileEditorMode::Create => (
                    ProfileEditorDraft::new(daemon_is_local),
                    StoredCredentialState::NotStored,
                ),
            };
            Ok(ProfileEditorSession {
                mode,
                draft,
                client_credential,
                daemon_is_local,
            })
        })
        .await
        .context("Profile editor task stopped unexpectedly")
        .and_then(|result| result);

        match result {
            Ok(session) => {
                RuntimeResult::presentation(PresentationRequest::ProfileEditor(Box::new(session)))
            }
            Err(error) => RuntimeResult::event(ControllerEvent::RuntimeError {
                profile_id: Some(profile_id),
                message: error.to_string(),
            }),
        }
    }

    async fn confirm_delete(&self, profile_id: Uuid) -> RuntimeResult {
        let result = tokio::task::spawn_blocking(move || {
            load_profile_by_id(&profile_id)
                .with_context(|| format!("Failed to load profile {profile_id}"))
        })
        .await
        .context("Delete confirmation task stopped unexpectedly")
        .and_then(|result| result);

        match result {
            Ok(profile) => RuntimeResult::presentation(PresentationRequest::ConfirmDelete(
                ProfileDeletionRequest {
                    profile_id,
                    profile_name: profile.metadata.name,
                },
            )),
            Err(error) => operation_failure(profile_id, ProfileOperation::Delete, error),
        }
    }

    async fn save_profile(
        &self,
        request: ProfileSaveRequest,
        offer_reconnect: bool,
    ) -> RuntimeResult {
        let profile_id = request.profile.metadata.id;
        let daemon_is_local = self.daemon_is_local();
        let result = tokio::task::spawn_blocking(move || {
            save_profile_with_credential(request, daemon_is_local)
        })
        .await
        .context("Profile save task stopped unexpectedly")
        .and_then(|result| result);
        match result {
            Ok(profile) => {
                let presentation = offer_reconnect.then(|| {
                    PresentationRequest::ConfirmReconnect(ProfileReconnectRequest {
                        profile_id,
                        profile_name: profile.metadata.name.clone(),
                    })
                });
                RuntimeResult {
                    events: vec![ControllerEvent::ProfileSaved(Box::new(profile))],
                    presentation,
                }
            }
            Err(error) => operation_failure(profile_id, ProfileOperation::Save, error),
        }
    }

    async fn delete_profile(&self, profile_id: Uuid) -> RuntimeResult {
        let daemon_is_local = self.daemon_is_local();
        let result = tokio::task::spawn_blocking(move || {
            delete_profile_with_credential(profile_id, daemon_is_local)
        })
        .await
        .context("Profile deletion task stopped unexpectedly")
        .and_then(|result| result);
        match result {
            Ok(()) => RuntimeResult::event(ControllerEvent::ProfileDeleted(profile_id)),
            Err(error) => operation_failure(profile_id, ProfileOperation::Delete, error),
        }
    }

    fn set_auto_reconnect(&self, profile_id: Uuid, enabled: bool) -> RuntimeResult {
        let result = (|| -> Result<Profile> {
            let mut profile = load_profile_by_id(&profile_id)
                .with_context(|| format!("Failed to load profile {profile_id}"))?;
            profile.options.auto_reconnect = enabled;
            profile.metadata.modified_at = Utc::now();
            validate_profile(&profile)?;
            save_profile(&profile, true)?;
            Ok(profile)
        })();

        match result {
            Ok(profile) => RuntimeResult::event(ControllerEvent::ProfileSaved(Box::new(profile))),
            Err(error) => {
                operation_failure(profile_id, ProfileOperation::ToggleAutoReconnect, error)
            }
        }
    }

    async fn start_profile(&self, profile_id: Uuid, operation: ProfileOperation) -> RuntimeResult {
        self.clear_client_credential_offer(profile_id);
        let result = match load_profile_by_id(&profile_id)
            .with_context(|| format!("Failed to load profile {profile_id}"))
        {
            Ok(profile) => self.daemon_client.start_tunnel(&profile).await,
            Err(error) => Err(error),
        };

        match result {
            // The pending controller operation is completed by a structured
            // SSE status event, not by the endpoint's display message.
            Ok(()) => RuntimeResult::default(),
            Err(error) => operation_failure(profile_id, operation, error),
        }
    }

    async fn stop_profile(&self, profile_id: Uuid, operation: ProfileOperation) -> RuntimeResult {
        match self.daemon_client.stop_tunnel(profile_id).await {
            // As with start, wait for the structured SSE state transition.
            Ok(()) => RuntimeResult::default(),
            Err(error) => operation_failure(profile_id, operation, error),
        }
    }

    async fn reconnect_profile(&self, profile_id: Uuid) -> RuntimeResult {
        self.clear_client_credential_offer(profile_id);
        let result = async {
            let current = self.daemon_client.get_tunnel_status(profile_id).await?;
            if current
                .as_ref()
                .is_some_and(|snapshot| profile_changes_require_reconnect(&snapshot.status))
            {
                self.daemon_client.stop_tunnel(profile_id).await?;
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        let current = self.daemon_client.get_tunnel_status(profile_id).await?;
                        if current.as_ref().is_none_or(|snapshot| {
                            !profile_changes_require_reconnect(&snapshot.status)
                        }) {
                            return Ok::<(), anyhow::Error>(());
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
                .await
                .context("Timed out waiting for the tunnel to stop before reconnecting")??;
            }

            let profile = load_profile_by_id(&profile_id)
                .with_context(|| format!("Failed to load profile {profile_id}"))?;
            self.daemon_client.start_tunnel(&profile).await
        }
        .await;

        match result {
            // Completion is still driven by structured SSE status events. The
            // polling above only sequences stop before start safely.
            Ok(()) => RuntimeResult::default(),
            Err(error) => operation_failure(profile_id, ProfileOperation::Reconnect, error),
        }
    }

    fn daemon_is_local(&self) -> bool {
        self.daemon_client.config.connection_mode == ConnectionMode::UnixSocket
    }

    /// Forget that a stored credential was offered, so the next start may offer it again.
    ///
    /// Deliberately keyed on the profile and cleared on `Starting`, not on the request id: the
    /// point is that a credential the server *rejected* must not be replayed against the new
    /// prompt the rejection produces, which carries a different id. Replaying it would burn
    /// through the server's `MaxAuthTries` without the user ever being asked. This mirrors
    /// `ClientHeldCredential::spent` on the CLI side.
    ///
    /// Duplicate *delivery* of one prompt is a separate concern, handled where prompts are
    /// recorded — see `AppCore::add_pending_auth`.
    fn clear_client_credential_offer(&self, profile_id: Uuid) {
        if let Ok(mut offered) = self.client_credentials_offered.lock() {
            offered.remove(&profile_id);
        }
    }

    async fn answer_client_credential(&self, request: &AuthRequest) -> ClientCredentialAnswer {
        if !matches!(
            request.auth_type,
            AuthRequestType::Password | AuthRequestType::KeyPassphrase
        ) {
            return ClientCredentialAnswer::NotApplicable;
        }

        let profile_id = request.tunnel_id;
        if self
            .client_credentials_offered
            .lock()
            .map(|offered| offered.contains(&profile_id))
            .unwrap_or(true)
        {
            return ClientCredentialAnswer::NotApplicable;
        }

        let daemon_is_local = self.daemon_is_local();
        let request_type = request.auth_type.clone();
        let lookup = tokio::task::spawn_blocking(move || -> Result<Option<SecretValue>> {
            let profile = load_profile_by_id(&profile_id)
                .with_context(|| format!("Failed to load profile {profile_id}"))?;
            if !profile_uses_client_credential(&profile, daemon_is_local, &request_type) {
                return Ok(None);
            }
            Ok(get_password(&profile_id).ok().map(SecretValue::new))
        })
        .await;

        let Ok(Ok(Some(secret))) = lookup else {
            return ClientCredentialAnswer::NotApplicable;
        };

        if let Ok(mut offered) = self.client_credentials_offered.lock() {
            offered.insert(profile_id);
        } else {
            return ClientCredentialAnswer::NotApplicable;
        }

        match self
            .daemon_client
            .submit_auth_with_id(profile_id, request.id, secret.expose().to_string())
            .await
        {
            Ok(()) => ClientCredentialAnswer::Submitted,
            Err(error) => ClientCredentialAnswer::SubmissionFailed(format!(
                "The stored client credential could not be submitted: {error}"
            )),
        }
    }
}

#[derive(Debug)]
enum ClientCredentialAnswer {
    NotApplicable,
    Submitted,
    SubmissionFailed(String),
}

enum CredentialSnapshot {
    Missing,
    Present(SecretValue),
}

fn client_credential_state(profile_id: Uuid) -> StoredCredentialState {
    match has_password(&profile_id) {
        Ok(true) => StoredCredentialState::Stored,
        Ok(false) => StoredCredentialState::NotStored,
        Err(error) => StoredCredentialState::Unavailable(error.to_string()),
    }
}

fn credential_snapshot(profile_id: Uuid) -> Result<CredentialSnapshot> {
    if has_password(&profile_id)? {
        Ok(CredentialSnapshot::Present(SecretValue::new(get_password(
            &profile_id,
        )?)))
    } else {
        Ok(CredentialSnapshot::Missing)
    }
}

fn restore_credential(profile_id: Uuid, snapshot: &CredentialSnapshot) -> Result<()> {
    match snapshot {
        CredentialSnapshot::Missing => remove_password(&profile_id)?,
        CredentialSnapshot::Present(secret) => store_password(&profile_id, secret.expose())?,
    }
    Ok(())
}

fn apply_credential_update(profile_id: Uuid, update: &CredentialUpdate) -> Result<()> {
    match update {
        CredentialUpdate::Keep => {}
        CredentialUpdate::Store(secret) => {
            if secret.is_empty() {
                anyhow::bail!("The credential cannot be empty");
            }
            store_password(&profile_id, secret.expose())?;
        }
        CredentialUpdate::Remove => remove_password(&profile_id)?,
    }
    Ok(())
}

fn save_profile_with_credential(
    mut request: ProfileSaveRequest,
    daemon_is_local: bool,
) -> Result<Profile> {
    let profile_id = request.profile.metadata.id;
    let previous = if request.overwrite {
        Some(
            load_profile_by_id(&profile_id)
                .with_context(|| format!("Failed to load profile {profile_id}"))?,
        )
    } else {
        None
    };
    let previous_storage = previous
        .as_ref()
        .map(|profile| {
            profile
                .connection
                .password_storage
                .resolved(daemon_is_local)
        })
        .unwrap_or(PasswordStorage::None);

    request.profile.connection.password_storage = request
        .profile
        .connection
        .password_storage
        .resolved(daemon_is_local);
    let target_storage = request.profile.connection.password_storage;

    match target_storage {
        PasswordStorage::Keychain => unreachable!("legacy storage was resolved above"),
        PasswordStorage::DaemonHost | PasswordStorage::File => {
            if previous_storage != target_storage
                || !matches!(request.credential, CredentialUpdate::Keep)
            {
                anyhow::bail!(
                    "Changing credentials for {} storage is WIP; the existing setting can only be preserved",
                    if target_storage == PasswordStorage::DaemonHost {
                        "daemon-host"
                    } else {
                        "file"
                    }
                );
            }
        }
        PasswordStorage::Client => {
            if matches!(request.credential, CredentialUpdate::Remove) {
                anyhow::bail!(
                    "A profile cannot use client storage after its client credential is removed"
                );
            }
            if matches!(request.credential, CredentialUpdate::Keep) && !has_password(&profile_id)? {
                anyhow::bail!("Enter a credential before enabling client storage");
            }
        }
        PasswordStorage::None => {
            if matches!(request.credential, CredentialUpdate::Store(_)) {
                anyhow::bail!("Select client storage before storing a credential");
            }
        }
    }

    if previous_storage == PasswordStorage::Client
        && target_storage != PasswordStorage::Client
        && !matches!(request.credential, CredentialUpdate::Remove)
    {
        anyhow::bail!("Removing client storage must also remove its credential");
    }

    validate_profile(&request.profile)?;
    if daemon_is_local && request.profile.connection.auth_type == AuthType::Key {
        let key_path = request
            .profile
            .connection
            .key_path
            .as_deref()
            .context("SSH key path is required for key authentication")?;
        validate_ssh_key_file(key_path)?;
        if let CredentialUpdate::Store(secret) = &request.credential {
            if !is_key_encrypted(key_path)? {
                anyhow::bail!("This SSH key is not encrypted, so it has no passphrase to store");
            }
            validate_key_passphrase(key_path, secret.expose())?;
        }
    }

    let previous_credential = if matches!(
        request.credential,
        CredentialUpdate::Store(_) | CredentialUpdate::Remove
    ) {
        Some(credential_snapshot(profile_id)?)
    } else {
        None
    };

    apply_credential_update(profile_id, &request.credential)?;
    if let Err(save_error) = save_profile(&request.profile, request.overwrite) {
        if let Some(snapshot) = &previous_credential {
            if let Err(rollback_error) = restore_credential(profile_id, snapshot) {
                anyhow::bail!(
                    "Failed to save the profile: {save_error}; credential rollback also failed: {rollback_error}"
                );
            }
        }
        return Err(save_error);
    }

    Ok(request.profile)
}

fn delete_profile_with_credential(profile_id: Uuid, daemon_is_local: bool) -> Result<()> {
    let profile = load_profile_by_id(&profile_id)
        .with_context(|| format!("Failed to load profile {profile_id}"))?;
    let client_held = profile
        .connection
        .password_storage
        .resolved(daemon_is_local)
        .is_client_held();

    let previous_credential = if client_held {
        let snapshot = credential_snapshot(profile_id)?;
        remove_password(&profile_id)?;
        Some(snapshot)
    } else {
        None
    };

    if let Err(delete_error) = delete_profile(profile_id) {
        if let Some(snapshot) = &previous_credential {
            if let Err(rollback_error) = restore_credential(profile_id, snapshot) {
                anyhow::bail!(
                    "Failed to delete the profile: {delete_error}; credential rollback also failed: {rollback_error}"
                );
            }
        }
        return Err(delete_error);
    }

    Ok(())
}

fn operation_failure(
    profile_id: Uuid,
    operation: ProfileOperation,
    error: impl std::fmt::Display,
) -> RuntimeResult {
    RuntimeResult::event(ControllerEvent::ProfileOperationFinished {
        profile_id,
        operation,
        outcome: OperationOutcome::Failed(error.to_string()),
    })
}

fn duplicate_name(source_name: &str, profiles: &[Profile]) -> String {
    let used_names: std::collections::HashSet<String> = profiles
        .iter()
        .map(|profile| profile.metadata.name.to_lowercase())
        .collect();
    let first = format!("{source_name} copy");
    if !used_names.contains(&first.to_lowercase()) {
        return first;
    }

    for suffix in 2_u32.. {
        let candidate = format!("{source_name} copy {suffix}");
        if !used_names.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }

    unreachable!("an unused numeric copy suffix always exists")
}

/// Map a daemon SSE event to controller events using variants and identifiers
/// only. Free-form daemon text is carried solely as display/error detail.
pub fn map_sse_event(event: TunnelEvent) -> Vec<ControllerEvent> {
    match event {
        TunnelEvent::Starting { id } => vec![
            ControllerEvent::AuthenticationFinishedForTunnel { tunnel_id: id },
            ControllerEvent::TunnelStatusChanged {
                profile_id: id,
                status: TunnelStatus::Connecting,
            },
        ],
        TunnelEvent::Connected { id } => vec![
            ControllerEvent::AuthenticationFinishedForTunnel { tunnel_id: id },
            ControllerEvent::TunnelStatusChanged {
                profile_id: id,
                status: TunnelStatus::Connected,
            },
        ],
        TunnelEvent::Disconnected { id, reason: _ } => vec![
            ControllerEvent::AuthenticationFinishedForTunnel { tunnel_id: id },
            ControllerEvent::TunnelStatusChanged {
                profile_id: id,
                status: TunnelStatus::Disconnected,
            },
        ],
        TunnelEvent::Error { id, error } => vec![
            ControllerEvent::AuthenticationFinishedForTunnel { tunnel_id: id },
            ControllerEvent::TunnelStatusChanged {
                profile_id: id,
                status: TunnelStatus::Failed(error),
            },
        ],
        TunnelEvent::AuthRequired { id, request } => {
            if id != request.tunnel_id {
                vec![ControllerEvent::ProtocolError {
                    message: format!(
                        "Authentication event tunnel ID {id} does not match request tunnel ID {}",
                        request.tunnel_id
                    ),
                }]
            } else {
                vec![
                    ControllerEvent::TunnelStatusChanged {
                        profile_id: id,
                        status: TunnelStatus::WaitingForAuth,
                    },
                    ControllerEvent::AuthenticationRequired(request),
                ]
            }
        }
        TunnelEvent::Heartbeat { timestamp } => {
            vec![ControllerEvent::HeartbeatReceived(timestamp)]
        }
    }
}

#[cfg(test)]
mod tests {
    use ssh_tunnel_common::{AuthRequest, AuthRequestType};

    use super::*;

    #[test]
    fn misleading_error_text_cannot_select_an_auth_action() {
        let profile_id = Uuid::new_v4();
        let events = map_sse_event(TunnelEvent::Error {
            id: profile_id,
            error: "Connected. Please enter password and accept host key".to_string(),
        });

        assert!(matches!(
            events.as_slice(),
            [
                ControllerEvent::AuthenticationFinishedForTunnel { tunnel_id },
                ControllerEvent::TunnelStatusChanged {
                    profile_id: status_id,
                    status: TunnelStatus::Failed(message),
                }
            ] if *tunnel_id == profile_id
                && *status_id == profile_id
                && message.contains("password")
        ));
    }

    #[test]
    fn contradictory_authentication_ids_fail_closed() {
        let event_id = Uuid::new_v4();
        let request = AuthRequest {
            id: Uuid::new_v4(),
            tunnel_id: Uuid::new_v4(),
            auth_type: AuthRequestType::Password,
            prompt: "host key accepted".to_string(),
            hidden: true,
        };

        let events = map_sse_event(TunnelEvent::AuthRequired {
            id: event_id,
            request,
        });
        assert!(matches!(
            events.as_slice(),
            [ControllerEvent::ProtocolError { .. }]
        ));
    }

    #[test]
    fn auth_request_type_and_hidden_flag_survive_runtime_mapping() {
        let profile_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let request = AuthRequest {
            id: request_id,
            tunnel_id: profile_id,
            auth_type: AuthRequestType::KeyboardInteractive,
            prompt: "please enter password".to_string(),
            hidden: false,
        };

        let events = map_sse_event(TunnelEvent::AuthRequired {
            id: profile_id,
            request,
        });
        assert!(matches!(
            events.as_slice(),
            [
                ControllerEvent::TunnelStatusChanged {
                    status: TunnelStatus::WaitingForAuth,
                    ..
                },
                ControllerEvent::AuthenticationRequired(mapped),
            ] if mapped.id == request_id
                && mapped.auth_type == AuthRequestType::KeyboardInteractive
                && !mapped.hidden
        ));
    }

    #[test]
    fn duplicate_names_are_deterministic_and_case_insensitive() {
        let source = test_profile("Work");
        let mut first_copy = test_profile("work COPY");
        first_copy.metadata.id = Uuid::new_v4();
        assert_eq!(duplicate_name("Work", &[source, first_copy]), "Work copy 2");
    }

    #[test]
    fn client_credential_eligibility_uses_only_storage_location_and_request_code() {
        let mut profile = test_profile("Work");
        profile.connection.password_storage = PasswordStorage::Client;

        assert!(profile_uses_client_credential(
            &profile,
            false,
            &AuthRequestType::Password
        ));
        assert!(profile_uses_client_credential(
            &profile,
            true,
            &AuthRequestType::KeyPassphrase
        ));
        assert!(!profile_uses_client_credential(
            &profile,
            true,
            &AuthRequestType::TwoFactorCode
        ));
        assert!(!profile_uses_client_credential(
            &profile,
            true,
            &AuthRequestType::KeyboardInteractive
        ));
        assert!(!profile_uses_client_credential(
            &profile,
            true,
            &AuthRequestType::HostKeyVerification
        ));
    }

    #[test]
    fn legacy_keychain_resolution_uses_structured_connection_location() {
        let mut profile = test_profile("Legacy");
        profile.connection.password_storage = PasswordStorage::Keychain;

        assert!(!profile_uses_client_credential(
            &profile,
            true,
            &AuthRequestType::Password
        ));
        assert!(profile_uses_client_credential(
            &profile,
            false,
            &AuthRequestType::Password
        ));
    }

    fn test_profile(name: &str) -> Profile {
        use std::path::PathBuf;

        use ssh_tunnel_common::{
            AuthType, ConnectionConfig, ForwardingConfig, ForwardingType, PasswordStorage,
        };

        Profile::new(
            name.to_string(),
            ConnectionConfig {
                host: "example.test".to_string(),
                port: 22,
                user: "user".to_string(),
                auth_type: AuthType::Key,
                key_path: Some(PathBuf::from("/tmp/test-key")),
                password_storage: PasswordStorage::None,
            },
            ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                local_port: Some(8080),
                remote_host: Some("localhost".to_string()),
                remote_port: Some(80),
                bind_address: "127.0.0.1".to_string(),
            },
        )
    }
}
