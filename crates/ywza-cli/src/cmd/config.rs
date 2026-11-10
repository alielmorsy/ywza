use anyhow::{bail, Result};
use clap::Subcommand;
use ywza_proto::{client::ask, AgentMsg, ClientMsg, Control};

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Print the agent config as TOML (api key masked).
    Get,
    /// Set LLM settings; omitted ones keep their current value.
    SetLlm {
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        api_key: Option<String>,
    },
}

pub async fn run(c: &mut Control, sub: ConfigCmd) -> Result<()> {
    let AgentMsg::Config { mut config } = ask(c, ClientMsg::GetConfig).await? else {
        bail!("unexpected reply to get_config");
    };
    match sub {
        ConfigCmd::Get => print!("{}", toml::to_string(&config)?),
        ConfigCmd::SetLlm {
            base_url,
            model,
            api_key,
        } => {
            config.llm.base_url = base_url.unwrap_or(config.llm.base_url);
            config.llm.model = model.unwrap_or(config.llm.model);
            config.llm.api_key = api_key.unwrap_or_default();
            // Any reply other than `error` means it took.
            ask(c, ClientMsg::SetConfig { config }).await?;
            println!("ok");
        }
    }
    Ok(())
}
