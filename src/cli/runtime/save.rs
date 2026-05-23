use anyhow::Result;
use clap::Args;
use slipbox_core::ExplorationArtifactMetadata;

#[derive(Debug, Clone, Args)]
pub(crate) struct SaveReviewArgs {
    /// Persist this live audit or workflow run as a durable review.
    #[arg(long = "save-review")]
    pub(crate) save_review: bool,
    /// Durable identifier to assign to the saved review.
    #[arg(long = "review-id", value_name = "ID")]
    pub(crate) review_id: Option<String>,
    /// Human title to assign to the saved review.
    #[arg(long = "review-title", value_name = "TITLE")]
    pub(crate) review_title: Option<String>,
    /// Optional human summary for the saved review.
    #[arg(long = "review-summary", value_name = "TEXT")]
    pub(crate) review_summary: Option<String>,
    /// Replace an existing saved review with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SaveArtifactArgs {
    /// Persist the live command as a durable exploration artifact.
    #[arg(long)]
    pub(crate) save: bool,
    /// Durable identifier to assign to the saved artifact.
    #[arg(long = "artifact-id", value_name = "ID")]
    pub(crate) artifact_id: Option<String>,
    /// Human title to assign to the saved artifact.
    #[arg(long = "artifact-title", value_name = "TITLE")]
    pub(crate) artifact_title: Option<String>,
    /// Optional human summary for the saved artifact.
    #[arg(long = "artifact-summary", value_name = "TEXT")]
    pub(crate) artifact_summary: Option<String>,
    /// Replace an existing saved artifact with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SaveReviewRequest {
    pub(crate) review_id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) overwrite: bool,
}

impl SaveReviewArgs {
    pub(crate) fn request(&self) -> Result<Option<SaveReviewRequest>> {
        let mut stray_flags = Vec::new();
        if self.review_id.is_some() {
            stray_flags.push("--review-id");
        }
        if self.review_title.is_some() {
            stray_flags.push("--review-title");
        }
        if self.review_summary.is_some() {
            stray_flags.push("--review-summary");
        }
        if self.overwrite {
            stray_flags.push("--overwrite");
        }

        if !self.save_review {
            if stray_flags.is_empty() {
                return Ok(None);
            }
            anyhow::bail!("{} require --save-review", render_flag_list(&stray_flags));
        }

        Ok(Some(SaveReviewRequest {
            review_id: self.review_id.clone(),
            title: self.review_title.clone(),
            summary: self.review_summary.clone(),
            overwrite: self.overwrite,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SaveArtifactRequest {
    pub(crate) metadata: ExplorationArtifactMetadata,
    pub(crate) overwrite: bool,
}

impl SaveArtifactArgs {
    pub(crate) fn request(&self) -> Result<Option<SaveArtifactRequest>> {
        let mut stray_flags = Vec::new();
        if self.artifact_id.is_some() {
            stray_flags.push("--artifact-id");
        }
        if self.artifact_title.is_some() {
            stray_flags.push("--artifact-title");
        }
        if self.artifact_summary.is_some() {
            stray_flags.push("--artifact-summary");
        }
        if self.overwrite {
            stray_flags.push("--overwrite");
        }

        if !self.save {
            if stray_flags.is_empty() {
                return Ok(None);
            }
            anyhow::bail!("{} require --save", render_flag_list(&stray_flags));
        }

        let mut missing_flags = Vec::new();
        if self.artifact_id.is_none() {
            missing_flags.push("--artifact-id");
        }
        if self.artifact_title.is_none() {
            missing_flags.push("--artifact-title");
        }
        if !missing_flags.is_empty() {
            anyhow::bail!("--save requires {}", render_flag_list(&missing_flags));
        }

        Ok(Some(SaveArtifactRequest {
            metadata: ExplorationArtifactMetadata {
                artifact_id: self
                    .artifact_id
                    .clone()
                    .expect("validated artifact_id should be present"),
                title: self
                    .artifact_title
                    .clone()
                    .expect("validated artifact_title should be present"),
                summary: self.artifact_summary.clone(),
            },
            overwrite: self.overwrite,
        }))
    }
}

pub(crate) fn render_flag_list(flags: &[&str]) -> String {
    match flags {
        [] => String::new(),
        [flag] => (*flag).to_owned(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let mut output = flags[..flags.len() - 1].join(", ");
            output.push_str(", and ");
            output.push_str(flags[flags.len() - 1]);
            output
        }
    }
}
