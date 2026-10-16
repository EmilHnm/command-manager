//! Reads `~/.ssh/config` for the "Nhập ~/.ssh/config" screen (SCR-07).
//!
//! Only a subset is understood: Host, HostName, Port, User, IdentityFile,
//! ProxyJump (one hop) and ForwardAgent. Everything else that changes how a
//! connection is made is reported per host, so the user can pick
//! "Dùng OpenSSH hệ thống" for it instead. Values from `Host *` blocks fill
//! in what a host leaves unset, as in OpenSSH (first value wins).

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ConfigHost {
    pub alias: String,
    pub host_name: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
    pub forward_agent: bool,
    /// Directives that were ignored, such as `ProxyCommand`.
    pub unsupported: Vec<String>,
}

#[derive(Default)]
struct Block {
    patterns: Vec<String>,
    host_name: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    identity_file: Option<String>,
    proxy_jump: Option<String>,
    forward_agent: Option<bool>,
    unsupported: Vec<String>,
}

const UNSUPPORTED: &[&str] = &[
    "proxycommand",
    "localforward",
    "remoteforward",
    "dynamicforward",
    "certificatefile",
    "pkcs11provider",
    "securitykeyprovider",
    "hostkeyalias",
    "controlmaster",
    "controlpath",
    "requesttty",
    "remotecommand",
];

pub fn parse(text: &str) -> Vec<ConfigHost> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut in_match = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((keyword, value)) = split_directive(line) else {
            continue;
        };
        let keyword = keyword.to_ascii_lowercase();
        match keyword.as_str() {
            "host" => {
                in_match = false;
                blocks.push(Block {
                    patterns: value.split_whitespace().map(str::to_string).collect(),
                    ..Default::default()
                });
                continue;
            }
            "match" => {
                in_match = true;
                continue;
            }
            _ => {}
        }
        if in_match {
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            // Directives before the first Host apply to every host.
            blocks.push(Block {
                patterns: vec!["*".into()],
                ..Default::default()
            });
            apply(blocks.last_mut().expect("just pushed"), &keyword, value);
            continue;
        };
        apply(block, &keyword, value);
    }

    let defaults: Vec<&Block> = blocks
        .iter()
        .filter(|block| block.patterns.iter().all(|p| p == "*"))
        .collect();

    let mut hosts: Vec<ConfigHost> = Vec::new();
    for block in &blocks {
        for alias in &block.patterns {
            if alias.contains(['*', '?', '!']) || hosts.iter().any(|h| &h.alias == alias) {
                continue;
            }
            let mut host = ConfigHost {
                alias: alias.clone(),
                host_name: block.host_name.clone(),
                port: block.port,
                user: block.user.clone(),
                identity_file: block.identity_file.clone(),
                proxy_jump: block.proxy_jump.clone(),
                forward_agent: block.forward_agent.unwrap_or(false),
                unsupported: block.unsupported.clone(),
            };
            for default in &defaults {
                host.host_name = host.host_name.or_else(|| default.host_name.clone());
                host.port = host.port.or(default.port);
                host.user = host.user.or_else(|| default.user.clone());
                host.identity_file = host.identity_file.or_else(|| default.identity_file.clone());
                host.proxy_jump = host.proxy_jump.or_else(|| default.proxy_jump.clone());
                if block.forward_agent.is_none() {
                    host.forward_agent = default.forward_agent.unwrap_or(host.forward_agent);
                }
                for item in &default.unsupported {
                    if !host.unsupported.contains(item) {
                        host.unsupported.push(item.clone());
                    }
                }
            }
            if host
                .proxy_jump
                .as_deref()
                .is_some_and(|jump| jump.contains(','))
            {
                host.unsupported.push("ProxyJump (nhiều cấp)".into());
            }
            hosts.push(host);
        }
    }
    hosts
}

fn split_directive(line: &str) -> Option<(&str, &str)> {
    let index = line.find(|c: char| c.is_whitespace() || c == '=')?;
    let keyword = &line[..index];
    let rest = line[index..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
    let rest = rest
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(rest);
    Some((keyword, rest))
}

fn apply(block: &mut Block, keyword: &str, value: &str) {
    // OpenSSH keeps the first value it sees for each option.
    match keyword {
        "hostname" => {
            block.host_name.get_or_insert_with(|| value.to_string());
        }
        "port" => {
            if block.port.is_none() {
                block.port = value.parse().ok().filter(|port| *port != 0);
            }
        }
        "user" => {
            block.user.get_or_insert_with(|| value.to_string());
        }
        "identityfile" => {
            block.identity_file.get_or_insert_with(|| value.to_string());
        }
        "proxyjump" => {
            if !value.eq_ignore_ascii_case("none") {
                block.proxy_jump.get_or_insert_with(|| value.to_string());
            }
        }
        "forwardagent" => {
            block
                .forward_agent
                .get_or_insert(value.eq_ignore_ascii_case("yes"));
        }
        other if UNSUPPORTED.contains(&other) => {
            let label = canonical(other);
            if !block.unsupported.iter().any(|item| item == label) {
                block.unsupported.push(label.to_string());
            }
        }
        _ => {}
    }
}

fn canonical(keyword: &str) -> &'static str {
    match keyword {
        "proxycommand" => "ProxyCommand",
        "localforward" => "LocalForward",
        "remoteforward" => "RemoteForward",
        "dynamicforward" => "DynamicForward",
        "certificatefile" => "CertificateFile",
        "pkcs11provider" => "PKCS11Provider",
        "securitykeyprovider" => "SecurityKeyProvider",
        "hostkeyalias" => "HostKeyAlias",
        "controlmaster" => "ControlMaster",
        "controlpath" => "ControlPath",
        "requesttty" => "RequestTTY",
        "remotecommand" => "RemoteCommand",
        _ => "?",
    }
}

/// Expands a leading `~` the way OpenSSH does for IdentityFile.
pub fn expand_home(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = super::known_hosts::home_dir() {
            return home.join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = super::known_hosts::home_dir() {
            return home;
        }
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hosts_defaults_and_warnings() {
        let text = r#"
# global
ServerAliveInterval 30

Host prod-api api
    HostName 10.0.4.12
    User deploy
    Port=2222
    IdentityFile "~/.ssh/prod key"
    ProxyJump bastion

Host bastion
    HostName bastion.example.com
    ProxyCommand nc %h %p

Host *.internal
    User nobody

Match host legacy
    User ignored

Host deep
    ProxyJump a,b

Host *
    User emil
    ForwardAgent yes
"#;
        let hosts = parse(text);
        let aliases: Vec<_> = hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(aliases, vec!["prod-api", "api", "bastion", "deep"]);

        let prod = &hosts[0];
        assert_eq!(prod.host_name.as_deref(), Some("10.0.4.12"));
        assert_eq!(prod.user.as_deref(), Some("deploy"));
        assert_eq!(prod.port, Some(2222));
        assert_eq!(prod.identity_file.as_deref(), Some("~/.ssh/prod key"));
        assert_eq!(prod.proxy_jump.as_deref(), Some("bastion"));
        assert!(prod.forward_agent);
        assert!(prod.unsupported.is_empty());

        let bastion = &hosts[2];
        assert_eq!(bastion.user.as_deref(), Some("emil"));
        assert_eq!(bastion.unsupported, vec!["ProxyCommand"]);

        assert_eq!(hosts[3].unsupported, vec!["ProxyJump (nhiều cấp)"]);
    }

    #[test]
    fn first_value_wins() {
        let hosts = parse("Host a\n  User one\n  User two\n");
        assert_eq!(hosts[0].user.as_deref(), Some("one"));
    }
}
