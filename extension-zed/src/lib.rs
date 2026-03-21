use zed_extension_api as zed;

struct AuwgentExtension;

impl zed::Extension for AuwgentExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        // Find the LSP binary. For now, we assume it's in the PATH.
        // In a production extension, you might download it from GitHub releases.
        let path = worktree
            .which("auwgent-lsp")
            .ok_or_else(|| "auwgent-lsp binary not found in PATH".to_string())?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: vec![],
        })
    }
}

zed::register_extension!(AuwgentExtension);
