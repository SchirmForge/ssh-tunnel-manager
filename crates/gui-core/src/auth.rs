// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Structured authentication queue and presentation models.

use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt;

use ssh_tunnel_common::{AuthRequest, AuthRequestType};
use uuid::Uuid;

/// The semantic kind of authentication UI to present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPromptKind {
    KeyPassphrase,
    Password,
    TwoFactorCode,
    KeyboardInteractive,
    HostKeyVerification,
}

impl From<&AuthRequestType> for AuthPromptKind {
    fn from(value: &AuthRequestType) -> Self {
        match value {
            AuthRequestType::KeyPassphrase => Self::KeyPassphrase,
            AuthRequestType::Password => Self::Password,
            AuthRequestType::TwoFactorCode => Self::TwoFactorCode,
            AuthRequestType::KeyboardInteractive => Self::KeyboardInteractive,
            AuthRequestType::HostKeyVerification => Self::HostKeyVerification,
        }
    }
}

/// Input control selected from structured request fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthInputMode {
    VisibleText,
    HiddenText,
    HostKeyDecision,
}

/// Presentation-ready authentication request.
#[derive(Debug, Clone)]
pub struct AuthPromptSnapshot {
    pub request_id: Uuid,
    pub tunnel_id: Uuid,
    /// Locally resolved profile label. Missing profiles fall back to the
    /// structured tunnel ID in the presentation adapter.
    pub profile_name: Option<String>,
    pub kind: AuthPromptKind,
    pub input_mode: AuthInputMode,
    /// Display-only daemon copy. This field must never select behavior.
    pub prompt: String,
}

impl From<&AuthRequest> for AuthPromptSnapshot {
    fn from(request: &AuthRequest) -> Self {
        let kind = AuthPromptKind::from(&request.auth_type);
        let input_mode = match request.auth_type {
            AuthRequestType::HostKeyVerification => AuthInputMode::HostKeyDecision,
            _ if request.hidden => AuthInputMode::HiddenText,
            _ => AuthInputMode::VisibleText,
        };

        Self {
            request_id: request.id,
            tunnel_id: request.tunnel_id,
            profile_name: None,
            kind,
            input_mode,
            prompt: request.prompt.clone(),
        }
    }
}

/// Typed answer emitted by a presentation adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthAnswer {
    Input(String),
    HostKeyDecision(bool),
}

/// Validated wire submission produced by the protocol adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSubmission {
    pub request_id: Uuid,
    pub tunnel_id: Uuid,
    pub response: String,
}

impl AuthSubmission {
    /// Validate an answer using only the request code, then serialize it for
    /// the current daemon protocol.
    pub fn from_request(
        request: &AuthRequest,
        answer: AuthAnswer,
    ) -> Result<Self, AuthAnswerError> {
        let response = match (&request.auth_type, answer) {
            (AuthRequestType::HostKeyVerification, AuthAnswer::HostKeyDecision(true)) => {
                "yes".to_string()
            }
            (AuthRequestType::HostKeyVerification, AuthAnswer::HostKeyDecision(false)) => {
                "no".to_string()
            }
            (AuthRequestType::HostKeyVerification, AuthAnswer::Input(_)) => {
                return Err(AuthAnswerError::HostKeyDecisionRequired)
            }
            (_, AuthAnswer::Input(value)) => value,
            (_, AuthAnswer::HostKeyDecision(_)) => return Err(AuthAnswerError::TextInputRequired),
        };

        Ok(Self {
            request_id: request.id,
            tunnel_id: request.tunnel_id,
            response,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthAnswerError {
    HostKeyDecisionRequired,
    TextInputRequired,
}

impl fmt::Display for AuthAnswerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostKeyDecisionRequired => {
                formatter.write_str("the request code requires a host-key decision")
            }
            Self::TextInputRequired => formatter.write_str("the request code requires text input"),
        }
    }
}

impl Error for AuthAnswerError {}

/// FIFO authentication queue keyed by daemon request ID.
#[derive(Debug, Default)]
pub struct AuthQueue {
    active: Option<AuthRequest>,
    pending: VecDeque<AuthRequest>,
    known_ids: HashSet<Uuid>,
}

impl AuthQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a request. Returns false for a duplicate request ID.
    pub fn enqueue(&mut self, request: AuthRequest) -> bool {
        if !self.known_ids.insert(request.id) {
            return false;
        }

        if self.active.is_none() {
            self.active = Some(request);
        } else {
            self.pending.push_back(request);
        }
        true
    }

    pub fn active(&self) -> Option<&AuthRequest> {
        self.active.as_ref()
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn contains(&self, request_id: Uuid) -> bool {
        self.known_ids.contains(&request_id)
    }

    /// Complete or remove a request and promote the next FIFO item when the
    /// active request is removed.
    pub fn remove(&mut self, request_id: Uuid) -> Option<AuthRequest> {
        if self.active.as_ref().map(|request| request.id) == Some(request_id) {
            let removed = self.active.take();
            self.known_ids.remove(&request_id);
            self.active = self.pending.pop_front();
            return removed;
        }

        let position = self
            .pending
            .iter()
            .position(|request| request.id == request_id)?;
        let removed = self.pending.remove(position);
        self.known_ids.remove(&request_id);
        removed
    }

    pub fn remove_for_tunnel(&mut self, tunnel_id: Uuid) -> Vec<AuthRequest> {
        let ids: Vec<Uuid> = self
            .active
            .iter()
            .chain(self.pending.iter())
            .filter(|request| request.tunnel_id == tunnel_id)
            .map(|request| request.id)
            .collect();

        ids.into_iter()
            .filter_map(|request_id| self.remove(request_id))
            .collect()
    }

    /// Retain only requests reported by the daemon's latest structured
    /// inventory. Returns the requests that were removed.
    pub fn retain_request_ids(&mut self, request_ids: &HashSet<Uuid>) -> Vec<AuthRequest> {
        let stale_ids: Vec<Uuid> = self
            .active
            .iter()
            .chain(self.pending.iter())
            .filter(|request| !request_ids.contains(&request.id))
            .map(|request| request.id)
            .collect();

        stale_ids
            .into_iter()
            .filter_map(|request_id| self.remove(request_id))
            .collect()
    }

    pub fn clear(&mut self) {
        self.active = None;
        self.pending.clear();
        self.known_ids.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(auth_type: AuthRequestType, prompt: &str, hidden: bool) -> AuthRequest {
        AuthRequest {
            id: Uuid::new_v4(),
            tunnel_id: Uuid::new_v4(),
            auth_type,
            prompt: prompt.to_string(),
            hidden,
        }
    }

    #[test]
    fn prompt_kind_never_depends_on_daemon_text() {
        let password = request(
            AuthRequestType::Password,
            "HOST KEY ACCEPT? This is deliberately misleading",
            true,
        );
        let snapshot = AuthPromptSnapshot::from(&password);
        assert_eq!(snapshot.kind, AuthPromptKind::Password);
        assert_eq!(snapshot.input_mode, AuthInputMode::HiddenText);

        let host_key = request(
            AuthRequestType::HostKeyVerification,
            "Please enter your password",
            true,
        );
        let snapshot = AuthPromptSnapshot::from(&host_key);
        assert_eq!(snapshot.kind, AuthPromptKind::HostKeyVerification);
        assert_eq!(snapshot.input_mode, AuthInputMode::HostKeyDecision);
    }

    #[test]
    fn structured_hidden_flag_controls_input_visibility() {
        let visible = request(AuthRequestType::KeyboardInteractive, "mot de passe", false);
        let hidden = request(
            AuthRequestType::KeyboardInteractive,
            "public response",
            true,
        );

        assert_eq!(
            AuthPromptSnapshot::from(&visible).input_mode,
            AuthInputMode::VisibleText
        );
        assert_eq!(
            AuthPromptSnapshot::from(&hidden).input_mode,
            AuthInputMode::HiddenText
        );
    }

    #[test]
    fn queue_is_fifo_and_deduplicates_request_ids() {
        let first = request(AuthRequestType::Password, "first", true);
        let second = request(AuthRequestType::TwoFactorCode, "second", false);
        let first_id = first.id;
        let second_id = second.id;
        let mut queue = AuthQueue::new();

        assert!(queue.enqueue(first.clone()));
        assert!(!queue.enqueue(first));
        assert!(queue.enqueue(second));
        assert_eq!(queue.active().map(|request| request.id), Some(first_id));
        assert_eq!(queue.pending_len(), 1);

        queue.remove(first_id);
        assert_eq!(queue.active().map(|request| request.id), Some(second_id));
        assert_eq!(queue.pending_len(), 0);
    }

    #[test]
    fn inventory_reconciliation_retains_only_reported_request_ids() {
        let first = request(AuthRequestType::Password, "first", true);
        let second = request(AuthRequestType::TwoFactorCode, "second", false);
        let first_id = first.id;
        let second_id = second.id;
        let mut queue = AuthQueue::new();
        queue.enqueue(first);
        queue.enqueue(second);

        let retained = HashSet::from([second_id]);
        let removed = queue.retain_request_ids(&retained);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].id, first_id);
        assert_eq!(queue.active().map(|request| request.id), Some(second_id));
        assert_eq!(queue.pending_len(), 0);
    }

    #[test]
    fn typed_answers_are_validated_by_request_code() {
        let host_key = request(
            AuthRequestType::HostKeyVerification,
            "anything at all",
            false,
        );
        let accepted = AuthSubmission::from_request(&host_key, AuthAnswer::HostKeyDecision(true))
            .expect("host-key decision should be valid");
        assert_eq!(accepted.response, "yes");

        assert_eq!(
            AuthSubmission::from_request(&host_key, AuthAnswer::Input("yes".to_string())),
            Err(AuthAnswerError::HostKeyDecisionRequired)
        );

        let password = request(AuthRequestType::Password, "host key", true);
        assert_eq!(
            AuthSubmission::from_request(&password, AuthAnswer::HostKeyDecision(true)),
            Err(AuthAnswerError::TextInputRequired)
        );
    }
}
