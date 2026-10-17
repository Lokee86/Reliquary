use crate::config_args::ConfigCommand;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "reliquary",
    version,
    about = "Repo-local Reliquary bring-up CLI"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "reliquary.cfg")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(name = "rel", visible_alias = "cva")]
    Rel {
        #[command(subcommand)]
        command: RelCommand,
    },
    Phy {
        #[command(subcommand)]
        command: PhyCommand,
    },
    Migrate {
        source: PathBuf,
        output: PathBuf,
    },
    MigrateConversationTitles {
        source: PathBuf,
        #[arg(value_name = "REL")]
        rel: PathBuf,
    },
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    Archive {
        #[command(subcommand)]
        command: ArchiveCommand,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Vectors {
        #[command(subcommand)]
        command: VectorsCommand,
    },
    Insomnia {
        #[command(subcommand)]
        command: InsomniaCommand,
    },
    Dev {
        #[command(subcommand)]
        command: DevCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum RelCommand {
    Create {
        path: PathBuf,
        #[arg(long = "type", value_name = "LABEL")]
        type_label: Option<String>,
    },
    Info {
        path: PathBuf,
    },
    Verify {
        path: PathBuf,
    },
    #[command(visible_alias = "vacuum")]
    Reclaim {
        path: PathBuf,
        output: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum PhyCommand {
    Create {
        path: PathBuf,
    },
    Info {
        path: PathBuf,
    },
    Verify {
        path: PathBuf,
    },
    #[command(visible_alias = "vacuum")]
    Reclaim {
        path: PathBuf,
        output: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum ImportCommand {
    GraphJsonl {
        input: PathBuf,
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_FRAGMENT_TURNS)]
        turns: usize,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_FRAGMENT_OVERLAP)]
        overlap: usize,
        #[arg(long)]
        leave_tail_open: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ArchiveCommand {
    Conversations {
        #[arg(value_name = "REL")]
        cva: PathBuf,
    },
    Show {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        conversation: String,
        #[arg(long)]
        branch: String,
    },
    Fragments {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        conversation: Option<String>,
    },
    Fragment {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        id: String,
    },
    Tombstone {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        conversation: String,
    },
    Lifecycle {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        conversation: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum VectorsCommand {
    Status {
        #[arg(value_name = "REL")]
        cva: PathBuf,
    },
    Profiles {
        #[arg(value_name = "REL")]
        cva: PathBuf,
    },
    Probe {
        #[arg(required = true, trailing_var_arg = true)]
        text: Vec<String>,
    },
    Build {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_REMOTE_EMBEDDING_BATCH_SIZE)]
        batch_size: usize,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_REMOTE_EMBEDDING_CONCURRENCY)]
        concurrency: usize,
    },
}

#[derive(Subcommand, Debug)]
pub enum InsomniaCommand {
    Run {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long, value_name = "PHY")]
        phy: Option<PathBuf>,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_INSOMNIA_WORKERS)]
        workers: usize,
        #[arg(long, default_value = reliquary_memory::DEFAULT_INSOMNIA_SCOPE)]
        scope: String,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_REMOTE_EMBEDDING_BATCH_SIZE)]
        embedding_batch_size: usize,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_REMOTE_EMBEDDING_CONCURRENCY)]
        embedding_concurrency: usize,
        #[arg(long)]
        existing_queue_only: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DevCommand {
    EstablishProfile {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        dimensions: u32,
        #[arg(long, default_value = "l2")]
        normalization: String,
        #[arg(long)]
        seed: u64,
        #[arg(long, default_value_t = 0.0)]
        drift: f32,
    },
    BuildVectors {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        seed: u64,
        #[arg(long, default_value_t = 0.0)]
        drift: f32,
    },
    Search {
        #[arg(value_name = "REL")]
        cva: PathBuf,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        seed: u64,
        #[arg(long, default_value_t = 0.0)]
        drift: f32,
        #[arg(long)]
        semantic_only: bool,
        #[arg(long, default_value_t = reliquary_memory::DEFAULT_SEARCH_RESULT_LIMIT)]
        limit: usize,
        #[arg(required = true, trailing_var_arg = true)]
        query: Vec<String>,
    },
}
