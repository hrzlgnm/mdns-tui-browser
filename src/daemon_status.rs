// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Snapshot of the mDNS daemon's local setup (sockets and multicast
//! memberships) and the problems derived from it.

use std::fmt;
use std::net::IpAddr;

use mdns_sd::{DaemonStatus, Membership, RunningState, SocketState};

/// An IP version the daemon runs a multicast socket for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpVersion {
    /// IPv4.
    V4,
    /// IPv6.
    V6,
}

impl IpVersion {
    /// Both versions, in display order.
    pub const ALL: [IpVersion; 2] = [IpVersion::V4, IpVersion::V6];

    /// Display label such as `IPv4`.
    pub fn label(self) -> &'static str {
        match self {
            IpVersion::V4 => "IPv4",
            IpVersion::V6 => "IPv6",
        }
    }
}

/// Multicast group membership of one IP version on an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipSnapshot {
    /// Addresses of this IP version on the interface.
    pub addrs: Vec<IpAddr>,
    /// Error from the last failed join attempt; `None` when joined.
    pub join_error: Option<String>,
}

impl From<&Membership> for MembershipSnapshot {
    fn from(membership: &Membership) -> Self {
        Self {
            addrs: membership.addrs().to_vec(),
            join_error: membership.join_error().map(str::to_string),
        }
    }
}

/// An interface the daemon has enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceSnapshot {
    /// Interface name, e.g. `eth0`.
    pub name: String,
    /// Interface index assigned by the OS.
    pub index: u32,
    /// IPv4 membership, `None` without IPv4 addresses.
    pub ipv4: Option<MembershipSnapshot>,
    /// IPv6 membership, `None` without IPv6 addresses.
    pub ipv6: Option<MembershipSnapshot>,
}

impl InterfaceSnapshot {
    /// The membership for `version`, if the interface has addresses of it.
    pub fn membership(&self, version: IpVersion) -> Option<&MembershipSnapshot> {
        match version {
            IpVersion::V4 => self.ipv4.as_ref(),
            IpVersion::V6 => self.ipv6.as_ref(),
        }
    }
}

/// Socket bind state and enabled interfaces of a running daemon.
///
/// Mirrors [`RunningState`], which cannot be constructed outside mdns-sd,
/// so the logic built on it stays testable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningSnapshot {
    /// Error from binding the IPv4 socket; `None` when bound.
    pub ipv4_bind_error: Option<String>,
    /// Error from binding the IPv6 socket; `None` when bound.
    pub ipv6_bind_error: Option<String>,
    /// Enabled interfaces, ordered by interface index.
    pub interfaces: Vec<InterfaceSnapshot>,
}

impl RunningSnapshot {
    /// The bind error of the socket for `version`, if binding failed.
    pub fn bind_error(&self, version: IpVersion) -> Option<&str> {
        match version {
            IpVersion::V4 => self.ipv4_bind_error.as_deref(),
            IpVersion::V6 => self.ipv6_bind_error.as_deref(),
        }
    }
}

fn bind_error(socket: &SocketState) -> Option<String> {
    socket.bind_error().map(str::to_string)
}

impl From<&RunningState> for RunningSnapshot {
    fn from(state: &RunningState) -> Self {
        Self {
            ipv4_bind_error: bind_error(state.ipv4_socket()),
            ipv6_bind_error: bind_error(state.ipv6_socket()),
            interfaces: state
                .enabled_intfs()
                .iter()
                .map(|intf| InterfaceSnapshot {
                    name: intf.name().to_string(),
                    index: intf.index(),
                    ipv4: intf.ipv4().map(MembershipSnapshot::from),
                    ipv6: intf.ipv6().map(MembershipSnapshot::from),
                })
                .collect(),
        }
    }
}

/// Something that keeps the daemon from discovering on all of its sockets
/// and interfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The daemon has shut down.
    Shutdown,
    /// The multicast socket of this IP version failed to bind.
    BindFailed(IpVersion),
    /// Joining the multicast group failed on an interface.
    JoinFailed {
        /// IP version of the failed membership.
        version: IpVersion,
        /// Name of the interface.
        interface: String,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Shutdown => write!(f, "mDNS daemon has shut down"),
            Problem::BindFailed(version) => write!(f, "{} socket failed to bind", version.label()),
            Problem::JoinFailed { version, interface } => {
                write!(
                    f,
                    "{} multicast join failed on {}",
                    version.label(),
                    interface
                )
            }
        }
    }
}

/// What the daemon reported about itself on the last status request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonSnapshot {
    /// The daemon runs with this setup.
    Running(RunningSnapshot),
    /// The daemon has shut down.
    Shutdown,
}

impl DaemonSnapshot {
    /// Converts a daemon status; statuses added by future mdns-sd versions
    /// yield `None`, leaving the previous snapshot in place.
    pub fn from_status(status: &DaemonStatus) -> Option<Self> {
        match status {
            DaemonStatus::Running(state) => Some(Self::Running(RunningSnapshot::from(state))),
            DaemonStatus::Shutdown => Some(Self::Shutdown),
            _ => None,
        }
    }

    /// Everything that keeps the daemon from discovering on all of its
    /// sockets and interfaces.
    ///
    /// Bind failures of an IP version the user disabled are expected (e.g.
    /// `--disable-ipv6` on a host without IPv6) and not reported.
    pub fn problems(&self, disable_ipv4: bool, disable_ipv6: bool) -> Vec<Problem> {
        let running = match self {
            DaemonSnapshot::Running(running) => running,
            DaemonSnapshot::Shutdown => return vec![Problem::Shutdown],
        };
        let mut problems = Vec::new();
        for (version, disabled) in [(IpVersion::V4, disable_ipv4), (IpVersion::V6, disable_ipv6)] {
            if !disabled && running.bind_error(version).is_some() {
                problems.push(Problem::BindFailed(version));
            }
        }
        for intf in &running.interfaces {
            for version in IpVersion::ALL {
                if intf
                    .membership(version)
                    .is_some_and(|m| m.join_error.is_some())
                {
                    problems.push(Problem::JoinFailed {
                        version,
                        interface: intf.name.clone(),
                    });
                }
            }
        }
        problems
    }
}

/// Ends status messages whose details the `m` key shows; the key handler
/// recognizes such messages by it.
pub const DETAILS_HINT: &str = "press m for details";

/// The status message to show when the daemon's problems change from
/// `previous` to `current`, or `None` when nothing worth announcing changed.
///
/// Announces newly appeared problems, and recovery once all are gone;
/// problems that persist or merely shrink stay silent so the 1s poll does
/// not keep re-raising the message.
pub fn problems_transition_message(previous: &[Problem], current: &[Problem]) -> Option<String> {
    let new: Vec<&Problem> = current.iter().filter(|p| !previous.contains(p)).collect();
    match new.split_first() {
        Some((first, rest)) => {
            let more = if rest.is_empty() {
                String::new()
            } else {
                format!(" (+{} more)", rest.len())
            };
            Some(format!("mDNS: {first}{more}; {DETAILS_HINT}"))
        }
        None if current.is_empty() && !previous.is_empty() => {
            Some("mDNS: all sockets and multicast joins are working again".to_string())
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn membership(addr: &str, join_error: Option<&str>) -> Option<MembershipSnapshot> {
        Some(MembershipSnapshot {
            addrs: vec![addr.parse().unwrap()],
            join_error: join_error.map(str::to_string),
        })
    }

    fn running(
        ipv4_bind_error: Option<&str>,
        ipv6_bind_error: Option<&str>,
        interfaces: Vec<InterfaceSnapshot>,
    ) -> DaemonSnapshot {
        DaemonSnapshot::Running(RunningSnapshot {
            ipv4_bind_error: ipv4_bind_error.map(str::to_string),
            ipv6_bind_error: ipv6_bind_error.map(str::to_string),
            interfaces,
        })
    }

    fn join_failed(version: IpVersion, interface: &str) -> Problem {
        Problem::JoinFailed {
            version,
            interface: interface.to_string(),
        }
    }

    #[test]
    fn test_problems_healthy_daemon_returns_empty() {
        let snapshot = running(
            None,
            None,
            vec![InterfaceSnapshot {
                name: "eth0".to_string(),
                index: 2,
                ipv4: membership("192.168.1.5", None),
                ipv6: membership("fe80::1", None),
            }],
        );
        assert!(snapshot.problems(false, false).is_empty());
    }

    #[test]
    fn test_problems_bind_and_join_failures_listed_in_order() {
        let snapshot = running(
            None,
            Some("Address family not supported"),
            vec![
                InterfaceSnapshot {
                    name: "eth0".to_string(),
                    index: 2,
                    ipv4: membership("192.168.1.5", Some("No such device")),
                    ipv6: None,
                },
                InterfaceSnapshot {
                    name: "wlan0".to_string(),
                    index: 3,
                    ipv4: membership("10.0.0.7", None),
                    ipv6: membership("fe80::2", Some("denied")),
                },
            ],
        );
        assert_eq!(
            snapshot.problems(false, false),
            vec![
                Problem::BindFailed(IpVersion::V6),
                join_failed(IpVersion::V4, "eth0"),
                join_failed(IpVersion::V6, "wlan0"),
            ]
        );
    }

    #[test]
    fn test_problems_bind_failure_of_disabled_version_ignored() {
        let snapshot = running(Some("v4 error"), Some("v6 error"), Vec::new());
        assert_eq!(
            snapshot.problems(false, true),
            vec![Problem::BindFailed(IpVersion::V4)]
        );
        assert_eq!(
            snapshot.problems(true, false),
            vec![Problem::BindFailed(IpVersion::V6)]
        );
    }

    #[test]
    fn test_problems_shutdown_reported() {
        assert_eq!(
            DaemonSnapshot::Shutdown.problems(false, false),
            vec![Problem::Shutdown]
        );
    }

    #[test]
    fn test_problem_display_describes_problem() {
        assert_eq!(Problem::Shutdown.to_string(), "mDNS daemon has shut down");
        assert_eq!(
            Problem::BindFailed(IpVersion::V6).to_string(),
            "IPv6 socket failed to bind"
        );
        assert_eq!(
            join_failed(IpVersion::V4, "eth0").to_string(),
            "IPv4 multicast join failed on eth0"
        );
    }

    #[test]
    fn test_transition_message_new_problem_announced() {
        assert_eq!(
            problems_transition_message(&[], &[join_failed(IpVersion::V4, "eth0")]),
            Some("mDNS: IPv4 multicast join failed on eth0; press m for details".to_string())
        );
    }

    #[test]
    fn test_transition_message_counts_additional_new_problems() {
        let previous = vec![join_failed(IpVersion::V6, "eth0")];
        let current = vec![
            Problem::BindFailed(IpVersion::V4),
            Problem::BindFailed(IpVersion::V6),
            join_failed(IpVersion::V4, "eth0"),
            join_failed(IpVersion::V6, "eth0"),
        ];
        assert_eq!(
            problems_transition_message(&previous, &current),
            Some("mDNS: IPv4 socket failed to bind (+2 more); press m for details".to_string())
        );
    }

    #[test]
    fn test_transition_message_persisting_or_shrinking_problems_silent() {
        let problems = vec![
            join_failed(IpVersion::V4, "eth0"),
            join_failed(IpVersion::V4, "wlan0"),
        ];
        assert_eq!(problems_transition_message(&problems, &problems), None);
        assert_eq!(problems_transition_message(&problems, &problems[..1]), None);
    }

    #[test]
    fn test_transition_message_recovery_announced() {
        assert_eq!(
            problems_transition_message(&[Problem::Shutdown], &[]),
            Some("mDNS: all sockets and multicast joins are working again".to_string())
        );
    }

    #[test]
    fn test_transition_message_healthy_stays_silent() {
        assert_eq!(problems_transition_message(&[], &[]), None);
    }
}
