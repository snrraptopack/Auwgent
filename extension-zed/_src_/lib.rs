use zed_extension_api as zed;

struct AuwgentExtension;

impl zed::Extension for AuwgentExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let path = "c:\\Users\\babyface\\Desktop\\auwgent\\Auwgent\\auwgent-compiler\\target\\release\\auwgent-lsp.exe".to_string();

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AuwgentExtension);
