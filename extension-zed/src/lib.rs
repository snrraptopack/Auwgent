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
        // 1. Check for a binary bundled alongside the extension (production installs)
        //    Zed extensions can ship a `bin/` folder — this is the preferred approach.
        // 2. Fall back to PATH lookup (works when the CLI is installed globally via npm)
        let binary_name = if cfg!(target_os = "windows") {
            "auwgent-lsp.exe"
        } else {
            "auwgent-lsp"
        };

        // Try to find the binary via the worktree environment PATH
        let path = worktree
            .which(binary_name)
            .ok_or_else(|| format!(
                "auwgent-lsp not found. Install it with: npm install -g @snrraptopack/auwgent-cli"
            ))?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AuwgentExtension);
