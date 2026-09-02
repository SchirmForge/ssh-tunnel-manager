// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Shell actions and their typed routing decisions.

use ssh_tunnel_gui_core::{AppCommand, FeatureAvailability, FeatureState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPage {
    Profiles,
    Daemon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellAction {
    Refresh,
    CreateProfile,
    OpenProfiles,
    OpenDaemon,
    ImportSshConfig,
    ShutdownDaemon,
    StartDaemon,
    RestartDaemon,
}

#[derive(Debug, Clone)]
pub enum ActionRoute {
    Dispatch(AppCommand),
    Navigate(ShellPage),
    Wip { reason: String },
    Unavailable { reason: String },
}

impl ShellAction {
    pub fn route(self, features: &FeatureAvailability) -> ActionRoute {
        match self {
            Self::Refresh => ActionRoute::Dispatch(AppCommand::Refresh),
            Self::CreateProfile => ActionRoute::Dispatch(AppCommand::CreateProfile),
            Self::OpenProfiles => ActionRoute::Navigate(ShellPage::Profiles),
            Self::OpenDaemon => ActionRoute::Navigate(ShellPage::Daemon),
            Self::ImportSshConfig => capability_route(
                &features.ssh_config_import,
                "No SSH import command is exposed by gui-core",
            ),
            Self::ShutdownDaemon => {
                capability_command_route(&features.daemon_shutdown, AppCommand::ShutdownDaemon)
            }
            Self::StartDaemon => capability_route(
                &features.daemon_start,
                "No daemon start command is exposed by gui-core",
            ),
            Self::RestartDaemon => capability_route(
                &features.daemon_restart,
                "No daemon restart command is exposed by gui-core",
            ),
        }
    }
}

fn capability_command_route(feature: &FeatureState, command: AppCommand) -> ActionRoute {
    match feature {
        FeatureState::Available => ActionRoute::Dispatch(command),
        FeatureState::UiOnlyWip { reason } => ActionRoute::Wip {
            reason: reason.clone(),
        },
        FeatureState::Unavailable { reason } => ActionRoute::Unavailable {
            reason: reason.clone(),
        },
    }
}

fn capability_route(feature: &FeatureState, missing_command: &str) -> ActionRoute {
    match feature {
        FeatureState::Available => ActionRoute::Unavailable {
            reason: missing_command.to_string(),
        },
        FeatureState::UiOnlyWip { reason } => ActionRoute::Wip {
            reason: reason.clone(),
        },
        FeatureState::Unavailable { reason } => ActionRoute::Unavailable {
            reason: reason.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implemented_shell_actions_dispatch_shared_commands() {
        let features = FeatureAvailability::default();

        assert!(matches!(
            ShellAction::Refresh.route(&features),
            ActionRoute::Dispatch(AppCommand::Refresh)
        ));
        assert!(matches!(
            ShellAction::CreateProfile.route(&features),
            ActionRoute::Dispatch(AppCommand::CreateProfile)
        ));
    }

    #[test]
    fn daemon_start_is_wip_even_when_its_reason_contains_action_words() {
        let features = FeatureAvailability {
            daemon_start: FeatureState::UiOnlyWip {
                reason: "Running and ready; restart now".to_string(),
            },
            ..FeatureAvailability::default()
        };

        assert!(matches!(
            ShellAction::StartDaemon.route(&features),
            ActionRoute::Wip { .. }
        ));
    }

    #[test]
    fn daemon_shutdown_obeys_the_structured_scope_capability() {
        let wip = FeatureAvailability::default();
        assert!(matches!(
            ShellAction::ShutdownDaemon.route(&wip),
            ActionRoute::Wip { .. }
        ));

        let available = FeatureAvailability {
            daemon_shutdown: FeatureState::Available,
            ..FeatureAvailability::default()
        };
        assert!(matches!(
            ShellAction::ShutdownDaemon.route(&available),
            ActionRoute::Dispatch(AppCommand::ShutdownDaemon)
        ));
    }

    #[test]
    fn an_unavailable_code_never_becomes_an_action_from_its_text() {
        let features = FeatureAvailability {
            ssh_config_import: FeatureState::Unavailable {
                reason: "Available: import this file".to_string(),
            },
            ..FeatureAvailability::default()
        };

        assert!(matches!(
            ShellAction::ImportSshConfig.route(&features),
            ActionRoute::Unavailable { .. }
        ));
    }
}
