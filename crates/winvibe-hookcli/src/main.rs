use clap::{Parser, Subcommand};
use winvibe_hookcli::commands::pre_tool_use::{read_stdin_payload, run_pre_tool_use};
use winvibe_hookcli::config_loader::{load_config_strict, resolve_config_path};
use winvibe_hookcli::exit_code::ExitCode;
use winvibe_hookcli::http_client::HookClient;

#[derive(Parser)]
#[command(name = "winvibe-hookcli")]
struct Cli {
    #[arg(long)]
    config: Option<std::path::PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 处理 PreToolUse 钩子事件
    PreToolUse {
        #[arg(long, default_value = "310")]
        max_time: u64,
    },
    /// 处理 Stop 钩子事件（无操作，直接退出）
    Stop,
}

fn main() {
    let cli = Cli::parse();
    let config_path = resolve_config_path(cli.config.as_deref());
    let config = match load_config_strict(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("winvibe-hookcli: config error: {e}");
            std::process::exit(ExitCode::ConfigError as i32);
        }
    };
    let base_url = format!("http://{}:{}", config.bind, config.port);
    let client = HookClient::new(base_url, config.auth_token.as_str().to_string());
    match cli.command {
        Commands::PreToolUse { max_time } => {
            let payload = match read_stdin_payload() {
                Ok(p) => p,
                Err(code) => code.exit(),
            };
            let code = run_pre_tool_use(&client, &payload, config.timeout_action, max_time);
            std::process::exit(code as i32);
        }
        Commands::Stop => {
            std::process::exit(0);
        }
    }
}
