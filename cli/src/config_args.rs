use clap::{Subcommand, ValueEnum};

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    Show,
    Verify,
    Credential {
        #[command(subcommand)]
        command: CredentialCommand,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum CredentialCommand {
    List,
    AddApiKey {
        id: String,
        #[arg(
            long,
            help = "Read the API key from one line on stdin instead of a hidden prompt"
        )]
        stdin: bool,
    },
    LoginCodex {
        id: String,
    },
    RefreshCodex {
        id: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModelCommand {
    SetGeneral {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_enum)]
        reasoning: Option<ReasoningArg>,
    },
    ClearGeneral,
    SetInsomnia {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_enum)]
        reasoning: Option<ReasoningArg>,
    },
    ClearInsomnia,
    SetInsomniaMetadata {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_enum)]
        reasoning: Option<ReasoningArg>,
    },
    ClearInsomniaMetadata,
    SetEntityExtraction {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_enum)]
        reasoning: Option<ReasoningArg>,
    },
    ClearEntityExtraction,
    SetEntityResolutionDecision {
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: String,
    },
    ClearEntityResolutionDecision,
    SetMemoryRetrievalDecision {
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: String,
    },
    ClearMemoryRetrievalDecision,
    SetEntityResolution {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_enum)]
        reasoning: Option<ReasoningArg>,
    },
    ClearEntityResolution,
    SetDream {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_enum)]
        reasoning: Option<ReasoningArg>,
    },
    ClearDream,
    SetEmbedding {
        #[arg(long, value_enum, default_value = "openai-ready")]
        provider: ProviderArg,
        #[arg(long)]
        model: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        dimensions: u32,
        #[arg(long, value_enum, default_value = "l2")]
        normalization: NormalizationArg,
    },
    ClearEmbedding,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ProviderArg {
    #[value(name = "openai-codex")]
    OpenAiCodex,
    #[value(name = "openai-ready")]
    OpenAiReady,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ReasoningArg {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum NormalizationArg {
    None,
    L2,
}
