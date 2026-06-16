use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "git-vacuum", version = "0.1.0", about = "Discover and sync GitHub repositories locally")]
pub struct Cli {
    #[arg(short, long, env = "GIT_VACUUM_TOKEN", help = "GitHub Personal Access Token")]
    pub token: Option<String>,

    #[arg(long, env = "GIT_VACUUM_PATH", default_value = "", help = "Clone destination path")]
    pub path: Option<String>,

    #[arg(long, env = "GIT_VACUUM_CONCURRENCY", default_value = "8", help = "Max concurrent git operations")]
    pub concurrency: Option<usize>,

    #[arg(long, env = "GIT_VACUUM_GITHUB_URL", help = "GitHub Enterprise base URL")]
    pub github_url: Option<String>,

    #[arg(long, help = "Run a single non-interactive sync and exit")]
    pub sync: bool,

    #[arg(long, help = "Use mirror mode (bare clones)")]
    pub mirror: bool,

    #[arg(long, help = "Include wiki repositories")]
    pub include_wikis: bool,

    #[arg(short, long, help = "Quiet mode - suppress non-error output")]
    pub quiet: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}
