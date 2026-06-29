use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use mft_core::app_config::AppConfig;
use mft_core::discovery::discover_for;
use mft_core::peer::initiator::{pull_all, push_paths};
use mft_core::peer::{PeerConfig, PeerNode};
use mft_core::transfer::{download_all, upload_paths};
use mft_protocol::crypto::PasswordRecord;
use mft_win_peer::{choose_connect_addr, format_peer, validate_paths};

#[derive(Debug, Parser)]
#[command(name = "mft-win-peer", about = "Minimal Windows-side peer for MFT")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, alias = "bind")]
        listen: Option<SocketAddr>,
    },
    Peer {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, alias = "bind")]
        listen: Option<SocketAddr>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        inbox: Option<PathBuf>,
        paths: Vec<PathBuf>,
    },
    Discover {
        #[arg(long, default_value_t = 3)]
        seconds: u64,
    },
    Send {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        connect: Option<SocketAddr>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, default_value_t = 3)]
        discover_seconds: u64,
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    Pull {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        connect: Option<SocketAddr>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        discover_seconds: u64,
    },
    Download {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        connect: Option<SocketAddr>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        discover_seconds: u64,
    },
    Upload {
        #[arg(long)]
        connect: Option<SocketAddr>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, default_value_t = 3)]
        discover_seconds: u64,
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            dir,
            password,
            name,
            listen,
        } => init(dir, password, name, listen).await,
        Commands::Peer {
            dir,
            name,
            listen,
            password,
            inbox,
            paths,
        } => {
            let config = config_for_runtime(dir, password, name, listen, inbox, paths).await?;
            run_peer(config).await
        }
        Commands::Discover { seconds } => {
            let peers = discover_for(Duration::from_secs(seconds)).await?;
            for peer in peers {
                println!("{}", format_peer(&peer));
            }
            Ok(())
        }
        Commands::Send {
            dir,
            to,
            connect,
            password,
            discover_seconds,
            paths,
        } => {
            validate_paths(&paths)?;
            let addr = resolve_peer_addr(connect, to.as_deref(), discover_seconds).await?;
            let password = transfer_password(dir, password).await?;
            let report = push_paths(addr, &password, &paths).await?;
            println!("sent {} file(s), {} byte(s)", report.files, report.bytes);
            Ok(())
        }
        Commands::Pull {
            dir,
            from,
            connect,
            password,
            out,
            discover_seconds,
        } => {
            let base_dir = AppConfig::resolve_base_dir(dir).await?;
            let out = out.unwrap_or_else(|| base_dir.join("received"));
            let addr = resolve_peer_addr(connect, from.as_deref(), discover_seconds).await?;
            let password = transfer_password(Some(base_dir), password).await?;
            let report = pull_all(addr, &password, out.clone()).await?;
            println!(
                "pulled {} file(s), {} byte(s) into {}",
                report.files,
                report.bytes,
                out.display()
            );
            Ok(())
        }
        Commands::Download {
            dir,
            connect,
            password,
            out,
            discover_seconds,
        } => {
            let base_dir = AppConfig::resolve_base_dir(dir).await?;
            let out = out.unwrap_or_else(|| base_dir.join("received"));
            let addr = connect_or_discover(connect, discover_seconds).await?;
            let password = transfer_password(Some(base_dir), password).await?;
            let report = download_all(addr, &password, out.clone()).await?;
            println!(
                "downloaded {} file(s), {} byte(s) into {}",
                report.files,
                report.bytes,
                out.display()
            );
            Ok(())
        }
        Commands::Upload {
            connect,
            password,
            discover_seconds,
            paths,
        } => {
            validate_paths(&paths)?;
            let addr = connect_or_discover(connect, discover_seconds).await?;
            let password = password_or_prompt(password)?;
            let report = upload_paths(addr, &password, &paths).await?;
            println!(
                "uploaded {} file(s), {} byte(s)",
                report.files, report.bytes
            );
            Ok(())
        }
    }
}

async fn init(
    dir: Option<PathBuf>,
    password: Option<String>,
    name: Option<String>,
    listen: Option<SocketAddr>,
) -> anyhow::Result<()> {
    let base_dir = AppConfig::resolve_base_dir(dir).await?;
    let password =
        password_from_arg_or_env(password).map_or_else(|| password_or_prompt(None), Ok)?;
    let config = AppConfig::new(
        name.unwrap_or_else(hostname_label),
        listen.unwrap_or_else(AppConfig::default_listen_addr),
        PasswordRecord::create(&password)?,
        base_dir,
    );
    config.save().await?;
    config.save_location().await?;
    print_config_summary("mftx initialized", &config);
    Ok(())
}

async fn config_for_runtime(
    dir: Option<PathBuf>,
    password: Option<String>,
    name: Option<String>,
    listen: Option<SocketAddr>,
    inbox: Option<PathBuf>,
    paths: Vec<PathBuf>,
) -> anyhow::Result<AppConfig> {
    let base_dir = AppConfig::resolve_base_dir(dir).await?;
    let (mut config, created) = match AppConfig::load_from_base(&base_dir).await {
        Ok(config) => (config, false),
        Err(_) => (
            AppConfig::new(
                name.clone().unwrap_or_else(hostname_label),
                AppConfig::default_listen_addr(),
                PasswordRecord::create(
                    &password_from_arg_or_env(password.clone())
                        .map_or_else(|| password_or_prompt(None), Ok)?,
                )?,
                base_dir,
            ),
            true,
        ),
    };

    if let Some(password) = password {
        config.password = PasswordRecord::create(&password)?;
    }
    if let Some(name) = name {
        config.device_name = name;
    }
    if let Some(listen) = listen {
        config.listen_addr = listen;
    }
    if let Some(inbox) = inbox {
        config.inbox_dir = inbox;
    }
    if !paths.is_empty() {
        config.share_dir = paths[0].clone();
    }
    if created {
        config.save().await?;
    } else {
        config.ensure_dirs().await?;
    }
    Ok(config)
}

async fn run_peer(config: AppConfig) -> anyhow::Result<()> {
    let share_paths = config.default_share_paths();
    let node = PeerNode::bind(PeerConfig::new(
        config.device_name.clone(),
        config.listen_addr,
        config.password.clone(),
        share_paths.clone(),
        config.inbox_dir.clone(),
    ))
    .await?;
    println!("MFT peer online");
    print_config_summary("using mftx home", &config);
    println!("listen: {}", node.addr());
    println!("share: {}", share_paths[0].display());
    println!("press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    drop(node);
    Ok(())
}

async fn resolve_peer_addr(
    connect: Option<SocketAddr>,
    peer_name: Option<&str>,
    discover_seconds: u64,
) -> anyhow::Result<SocketAddr> {
    if let Some(addr) = connect {
        return Ok(addr);
    }
    let peers = discover_for(Duration::from_secs(discover_seconds)).await?;

    let peer = if let Some(peer_name) = peer_name {
        let matches: Vec<_> = peers
            .iter()
            .filter(|peer| {
                peer.device_name == peer_name || peer.session_id.to_string() == peer_name
            })
            .collect();
        match matches.len() {
            0 => anyhow::bail!("no peer named {peer_name}; use --connect <ip:port>"),
            1 => matches[0],
            _ => anyhow::bail!("multiple peers named {peer_name}; use session id or --connect"),
        }
    } else {
        match peers.len() {
            0 => anyhow::bail!("no peer discovered; use --connect <ip:port>"),
            1 => &peers[0],
            _ => {
                let names = peers
                    .iter()
                    .map(|peer| peer.device_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!("multiple peers discovered ({names}); pass --to/--from <peer>")
            }
        }
    };

    peer.observed_addr
        .ok_or_else(|| anyhow::anyhow!("discovered peer has no observed address"))
}

async fn connect_or_discover(
    connect: Option<SocketAddr>,
    discover_seconds: u64,
) -> anyhow::Result<SocketAddr> {
    if connect.is_some() {
        return choose_connect_addr(connect, &[]);
    }
    let peers = discover_for(Duration::from_secs(discover_seconds)).await?;
    choose_connect_addr(None, &peers)
}

fn hostname_label() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "windows-mft".to_string())
}

async fn transfer_password(
    _dir: Option<PathBuf>,
    password: Option<String>,
) -> anyhow::Result<String> {
    if let Some(password) = password_from_arg_or_env(password) {
        return Ok(password);
    }
    password_or_prompt(None)
}

fn password_from_arg_or_env(password: Option<String>) -> Option<String> {
    password.or_else(|| std::env::var_os("MFTX_PASSWORD")?.into_string().ok())
}

fn password_or_prompt(password: Option<String>) -> anyhow::Result<String> {
    match password {
        Some(password) => Ok(password),
        None => Ok(rpassword::prompt_password("Password: ")?),
    }
}

fn print_config_summary(title: &str, config: &AppConfig) {
    println!("{title}:");
    println!("  name: {}", config.device_name);
    println!("  home: {}", config.base_dir.display());
    println!("  inbox: {}", config.inbox_dir.display());
    println!("  share: {}", config.share_dir.display());
    println!("  received: {}", config.received_dir.display());
}

#[cfg(test)]
mod tests {
    use super::{password_from_arg_or_env, Cli, Commands};
    use clap::Parser;

    #[test]
    fn parses_zero_arg_peer_for_default_mftx_home() {
        let cli = Cli::parse_from(["mft-win-peer", "peer"]);

        match cli.command {
            Commands::Peer {
                name,
                listen,
                inbox,
                paths,
                ..
            } => {
                assert!(name.is_none());
                assert!(listen.is_none());
                assert!(inbox.is_none());
                assert!(paths.is_empty());
            }
            other => panic!("expected peer command, got {other:?}"),
        }
    }

    #[test]
    fn parses_init_with_one_mftx_directory() {
        let cli = Cli::parse_from(["mft-win-peer", "init", "--dir", "C:\\mftx"]);

        match cli.command {
            Commands::Init { dir, .. } => {
                assert_eq!(dir.unwrap().to_string_lossy(), "C:\\mftx");
            }
            other => panic!("expected init command, got {other:?}"),
        }
    }

    #[test]
    fn parses_pull_without_out_for_default_received_dir() {
        let cli = Cli::parse_from(["mft-win-peer", "pull", "--from", "Haven-Mac"]);

        match cli.command {
            Commands::Pull { from, out, .. } => {
                assert_eq!(from.as_deref(), Some("Haven-Mac"));
                assert!(out.is_none());
            }
            other => panic!("expected pull command, got {other:?}"),
        }
    }

    #[test]
    fn transfer_password_prefers_cli_value_then_env_without_using_config_hash() {
        std::env::set_var("MFTX_PASSWORD", "env-secret");
        assert_eq!(
            password_from_arg_or_env(Some("cli-secret".to_string())).as_deref(),
            Some("cli-secret")
        );
        assert_eq!(
            password_from_arg_or_env(None).as_deref(),
            Some("env-secret")
        );
        std::env::remove_var("MFTX_PASSWORD");
        assert!(password_from_arg_or_env(None).is_none());
    }

    #[test]
    fn parses_peer_command() {
        let cli = Cli::parse_from([
            "mft-win-peer",
            "peer",
            "--name",
            "Lou-Win",
            "--listen",
            "127.0.0.1:48151",
            "--inbox",
            "C:\\Users\\Lou\\Downloads\\mft-inbox",
        ]);

        match cli.command {
            Commands::Peer {
                name,
                listen,
                inbox,
                ..
            } => {
                assert_eq!(name.as_deref(), Some("Lou-Win"));
                assert_eq!(listen.unwrap().port(), 48151);
                assert!(inbox.unwrap().to_string_lossy().contains("mft-inbox"));
            }
            other => panic!("expected peer command, got {other:?}"),
        }
    }

    #[test]
    fn parses_send_to_peer_command() {
        let cli = Cli::parse_from([
            "mft-win-peer",
            "send",
            "--to",
            "Haven-Mac",
            "C:\\Users\\Lou\\Desktop\\a.txt",
        ]);

        match cli.command {
            Commands::Send { to, paths, .. } => {
                assert_eq!(to.as_deref(), Some("Haven-Mac"));
                assert_eq!(paths.len(), 1);
            }
            other => panic!("expected send command, got {other:?}"),
        }
    }

    #[test]
    fn parses_pull_from_peer_command() {
        let cli = Cli::parse_from([
            "mft-win-peer",
            "pull",
            "--from",
            "Haven-Mac",
            "--out",
            "C:\\Users\\Lou\\Downloads\\from-mac",
        ]);

        match cli.command {
            Commands::Pull { from, out, .. } => {
                assert_eq!(from.as_deref(), Some("Haven-Mac"));
                assert!(out.unwrap().to_string_lossy().contains("from-mac"));
            }
            other => panic!("expected pull command, got {other:?}"),
        }
    }
}
