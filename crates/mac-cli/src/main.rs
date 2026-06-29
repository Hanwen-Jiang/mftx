use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use mft_core::app_config::AppConfig;
use mft_core::discovery::{broadcast_once, discover_for, DiscoveryBeacon};
use mft_core::peer::initiator::{pull_all, push_paths};
use mft_core::peer::{PeerConfig, PeerNode};
use mft_core::transfer::{download_all, upload_paths, TransferServer};
use mft_protocol::crypto::PasswordRecord;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use tokio::time;

#[derive(Debug, Parser)]
#[command(
    name = "mft",
    about = "Lightweight encrypted LAN file transfer for macOS"
)]
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
        password: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, alias = "bind")]
        listen: Option<SocketAddr>,
        #[arg(long)]
        inbox: Option<PathBuf>,
        paths: Vec<PathBuf>,
    },
    Tui {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, alias = "bind")]
        listen: Option<SocketAddr>,
        #[arg(long)]
        inbox: Option<PathBuf>,
        paths: Vec<PathBuf>,
    },
    Serve {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, alias = "listen")]
        bind: Option<SocketAddr>,
        #[arg(long)]
        inbox: Option<PathBuf>,
        paths: Vec<PathBuf>,
    },
    Send {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        connect: Option<SocketAddr>,
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
    Receive {
        #[arg(long, alias = "mftx-dir")]
        home: Option<PathBuf>,
        #[arg(long)]
        connect: SocketAddr,
        #[arg(long)]
        password: Option<String>,
        #[arg(long = "dir", aliases = ["out", "inbox"])]
        out: Option<PathBuf>,
    },
    Upload {
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        connect: SocketAddr,
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    Discover {
        #[arg(long, default_value_t = 3)]
        seconds: u64,
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
            password,
            name,
            listen,
            inbox,
            paths,
        } => {
            let config = config_for_runtime(dir, password, name, listen, inbox, paths).await?;
            run_peer(config).await
        }
        Commands::Tui {
            dir,
            password,
            listen,
            inbox,
            paths,
        } => {
            let config = config_for_runtime(dir, password, None, listen, inbox, paths).await?;
            run_tui(config).await
        }
        Commands::Serve {
            dir,
            password,
            bind,
            inbox,
            paths,
        } => {
            eprintln!("warning: `mft serve` is a legacy alias; prefer `mft peer`");
            let config = config_for_runtime(dir, password, None, bind, inbox, paths).await?;
            run_peer(config).await
        }
        Commands::Send {
            dir,
            password,
            to,
            connect,
            discover_seconds,
            paths,
        } => {
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
        Commands::Receive {
            home,
            connect,
            password,
            out,
        } => {
            let base_dir = AppConfig::resolve_base_dir(home).await?;
            let out = out.unwrap_or_else(|| base_dir.join("received"));
            let password = transfer_password(Some(base_dir), password).await?;
            let report = download_all(connect, &password, out.clone()).await?;
            println!(
                "received {} file(s), {} byte(s) into {}",
                report.files,
                report.bytes,
                out.display()
            );
            Ok(())
        }
        Commands::Upload {
            connect,
            password,
            paths,
        } => {
            let password = password_or_prompt(password)?;
            let report = upload_paths(connect, &password, &paths).await?;
            println!(
                "uploaded {} file(s), {} byte(s)",
                report.files, report.bytes
            );
            Ok(())
        }
        Commands::Discover { seconds } => {
            let peers = discover_for(Duration::from_secs(seconds)).await?;
            for peer in peers {
                println!(
                    "{}  {}  session={}  capabilities={}",
                    peer.device_name,
                    peer.observed_addr
                        .map(|addr| addr.to_string())
                        .unwrap_or_else(|| format!("unknown:{}", peer.port)),
                    peer.session_id,
                    peer.capabilities.join(",")
                );
            }
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

async fn run_tui(config: AppConfig) -> anyhow::Result<()> {
    let paths = config.default_share_paths();
    let server = TransferServer::bind(
        config.listen_addr,
        config.password.clone(),
        paths.clone(),
        config.inbox_dir.clone(),
    )
    .await?;
    let beacon = DiscoveryBeacon::new(
        config.device_id,
        config.device_name.clone(),
        server.addr(),
        vec![
            "receive".to_string(),
            "push".to_string(),
            "pull".to_string(),
            "encrypted".to_string(),
            "blake3".to_string(),
        ],
    );
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut interval = time::interval(Duration::from_secs(2));

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),
                    Constraint::Min(4),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("MFTX", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" encrypted P2P LAN transfer"),
                ]),
                Line::from(format!("name: {}", config.device_name)),
                Line::from(format!("home: {}", config.base_dir.display())),
                Line::from(format!("listening: {}", server.addr())),
                Line::from(format!("inbox: {}", config.inbox_dir.display())),
                Line::from(format!("received: {}", config.received_dir.display())),
            ])
            .block(Block::default().borders(Borders::ALL).title("Status"));
            frame.render_widget(header, chunks[0]);

            let items: Vec<ListItem> = paths
                .iter()
                .map(|path| ListItem::new(path.display().to_string()))
                .collect();
            frame.render_widget(
                List::new(items)
                    .block(Block::default().borders(Borders::ALL).title("Shared paths")),
                chunks[1],
            );

            frame.render_widget(
                Paragraph::new("Defaults can be changed with `mft init --dir <path>`. Press q or Ctrl+C to quit.")
                    .block(Block::default().borders(Borders::ALL)),
                chunks[2],
            );
        })?;

        tokio::select! {
            _ = interval.tick() => {
                let _ = broadcast_once(&beacon).await;
            }
            _ = tokio::signal::ctrl_c() => break,
            event = poll_key() => {
                if matches!(event?, Some(KeyCode::Char('q'))) {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    drop(server);
    Ok(())
}

async fn poll_key() -> anyhow::Result<Option<KeyCode>> {
    tokio::task::spawn_blocking(|| -> anyhow::Result<Option<KeyCode>> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                return Ok(Some(key.code));
            }
        }
        Ok(None)
    })
    .await?
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

fn hostname_label() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "mac-mft".to_string())
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
        let cli = Cli::parse_from(["mft", "peer"]);

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
        let cli = Cli::parse_from(["mft", "init", "--dir", "/tmp/mftx"]);

        match cli.command {
            Commands::Init { dir, .. } => {
                assert_eq!(dir.unwrap().to_string_lossy(), "/tmp/mftx");
            }
            other => panic!("expected init command, got {other:?}"),
        }
    }

    #[test]
    fn parses_pull_without_out_for_default_received_dir() {
        let cli = Cli::parse_from(["mft", "pull", "--from", "Lou-Win"]);

        match cli.command {
            Commands::Pull { from, out, .. } => {
                assert_eq!(from.as_deref(), Some("Lou-Win"));
                assert!(out.is_none());
            }
            other => panic!("expected pull command, got {other:?}"),
        }
    }

    #[test]
    fn parses_legacy_receive_dir_as_output_dir() {
        let cli = Cli::parse_from([
            "mft",
            "receive",
            "--connect",
            "127.0.0.1:48151",
            "--dir",
            "/tmp/legacy-inbox",
        ]);

        match cli.command {
            Commands::Receive { out, .. } => {
                assert_eq!(out.unwrap().to_string_lossy(), "/tmp/legacy-inbox");
            }
            other => panic!("expected receive command, got {other:?}"),
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
            "mft",
            "peer",
            "--name",
            "Haven-Mac",
            "--listen",
            "127.0.0.1:48151",
            "--inbox",
            "/tmp/mft-inbox",
            "/tmp/share",
        ]);

        match cli.command {
            Commands::Peer {
                name,
                listen,
                inbox,
                paths,
                ..
            } => {
                assert_eq!(name.as_deref(), Some("Haven-Mac"));
                assert_eq!(listen.unwrap().port(), 48151);
                assert_eq!(inbox.unwrap().to_string_lossy(), "/tmp/mft-inbox");
                assert_eq!(paths.len(), 1);
            }
            other => panic!("expected peer command, got {other:?}"),
        }
    }

    #[test]
    fn parses_send_to_peer_command() {
        let cli = Cli::parse_from(["mft", "send", "--to", "Lou-Win", "/tmp/a.txt"]);

        match cli.command {
            Commands::Send { to, paths, .. } => {
                assert_eq!(to.as_deref(), Some("Lou-Win"));
                assert_eq!(paths.len(), 1);
            }
            other => panic!("expected send command, got {other:?}"),
        }
    }

    #[test]
    fn parses_pull_from_peer_command() {
        let cli = Cli::parse_from([
            "mft",
            "pull",
            "--from",
            "Lou-Win",
            "--out",
            "/tmp/from-peer",
        ]);

        match cli.command {
            Commands::Pull { from, out, .. } => {
                assert_eq!(from.as_deref(), Some("Lou-Win"));
                assert_eq!(out.unwrap().to_string_lossy(), "/tmp/from-peer");
            }
            other => panic!("expected pull command, got {other:?}"),
        }
    }
}
