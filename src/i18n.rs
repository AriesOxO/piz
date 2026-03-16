/// Supported display languages
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    Zh,
    En,
    Ja,
}

impl Lang {
    pub fn from_code(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "en" => Lang::En,
            "ja" => Lang::Ja,
            _ => Lang::Zh,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
            Lang::Ja => "ja",
        }
    }
}

/// All translatable UI strings
pub struct T {
    // ── ui ──
    pub cached: &'static str,
    pub command_explanation: &'static str,
    pub diagnosis: &'static str,
    pub cancelled: &'static str,
    pub thinking: &'static str,
    pub analyzing: &'static str,

    // ── danger warnings ──
    pub danger_warning: &'static str,
    pub modify_warning: &'static str,

    // ── executor prompts ──
    pub confirm_dangerous: &'static str,
    pub yes_execute: &'static str,
    pub no_cancel: &'static str,
    pub edit_command: &'static str,
    pub execute: &'static str,
    pub cancel: &'static str,
    pub edit: &'static str,
    pub edit_prompt: &'static str,
    pub exit_code: &'static str,

    // ── fix ──
    pub no_piz_record: &'static str,
    pub last_from_history: &'static str,
    pub last_succeeded: &'static str,
    pub failed_command: &'static str,

    // ── auto-fix ──
    pub auto_fix_prompt: &'static str,
    pub auto_fix_attempting: &'static str,
    pub auto_fix_failed: &'static str,

    // ── config init ──
    pub wizard_title: &'static str,
    pub select_backend: &'static str,
    pub auto_confirm_prompt: &'static str,
    pub extra_backends: &'static str,
    pub config_saved: &'static str,
    pub config_validated: &'static str,
    pub config_edit_hint: &'static str,
    pub config_rerun_hint: &'static str,
    pub config_overwrite: &'static str,
    pub api_key_prompt: &'static str,
    pub model_prompt: &'static str,
    pub select_provider: &'static str,
    pub base_url_prompt: &'static str,
    pub custom_url_prompt: &'static str,
    pub add_openai: &'static str,
    pub add_claude: &'static str,
    pub add_gemini: &'static str,
    pub add_ollama: &'static str,
    pub ollama_host: &'static str,

    // ── chat mode ──
    pub chat_title: &'static str,
    pub chat_hint: &'static str,
    pub bye: &'static str,

    // ── chat commands ──
    pub chat_help_desc: &'static str,
    pub chat_clear_desc: &'static str,
    pub chat_history_desc: &'static str,
    pub chat_cleared: &'static str,
    pub chat_unknown_cmd: &'static str,

    // ── multi-candidate ──
    pub select_command: &'static str,

    // ── injection detection ──
    pub inject_env_exfiltration: &'static str,
    pub inject_base64_shell: &'static str,
    pub inject_reverse_shell: &'static str,
    pub inject_eval_remote: &'static str,
    pub inject_source_remote: &'static str,
    pub inject_overwrite_config: &'static str,
    pub inject_crontab_modify: &'static str,
    pub inject_download_execute: &'static str,
    pub inject_config_file_attack: &'static str,
}

pub fn t(lang: Lang) -> &'static T {
    match lang {
        Lang::Zh => &ZH,
        Lang::En => &EN,
        Lang::Ja => &JA,
    }
}

static ZH: T = T {
    cached: "(缓存命中)",
    command_explanation: "命令解释：",
    diagnosis: "诊断：",
    cancelled: "已取消。",
    thinking: "思考中...",
    analyzing: "分析中...",

    danger_warning: "危险命令！可能导致数据丢失或系统损坏。",
    modify_warning: "该命令会修改文件或系统设置。",

    confirm_dangerous: "⚠ 确定要执行这条危险命令吗？",
    yes_execute: "是，执行（我了解风险）",
    no_cancel: "否，取消",
    edit_command: "编辑命令",
    execute: "[Y] 执行",
    cancel: "[n] 取消",
    edit: "[e] 编辑",
    edit_prompt: "编辑命令",
    exit_code: "退出码",

    no_piz_record: "未找到 piz 执行记录，正在读取 shell 历史...",
    last_from_history: "历史命令：",
    last_succeeded: "上次命令执行成功，无需修复。",
    failed_command: "失败命令：",

    auto_fix_prompt: "命令执行失败，是否自动修复？",
    auto_fix_attempting: "正在分析错误并尝试修复...",
    auto_fix_failed: "自动修复失败，原因：",

    wizard_title: "⚙ piz 配置向导",
    select_backend: "选择默认 LLM 后端",

    auto_confirm_prompt: "安全命令是否自动执行（不弹出确认）？",
    extra_backends: "是否配置其他后端？",
    config_saved: "✔ 配置已保存：",
    config_validated: "✔ 配置验证通过。",
    config_edit_hint: "后续可手动编辑：",
    config_rerun_hint: "或重新运行：piz config --init",
    config_overwrite: "配置已存在，是否覆盖？",
    api_key_prompt: "API 密钥",
    model_prompt: "模型名称",
    select_provider: "选择 API 供应商",
    base_url_prompt: "API 地址",
    custom_url_prompt: "是否使用自定义 API 地址（代理）？",
    add_openai: "添加 OpenAI 兼容后端？",
    add_claude: "添加 Claude 后端？",
    add_gemini: "添加 Gemini 后端？",
    add_ollama: "添加 Ollama 后端？",
    ollama_host: "Ollama 地址",

    chat_title: "交互模式",
    chat_hint: "输入你的请求，或 'exit'/'quit' 退出。",
    bye: "再见！",

    chat_help_desc: "/help - 显示可用命令  /clear - 清除历史  /history - 查看历史",
    chat_clear_desc: "清除对话历史",
    chat_history_desc: "查看对话历史",
    chat_cleared: "对话历史已清除。",
    chat_unknown_cmd: "未知命令。输入 /help 查看可用命令。",

    select_command: "选择要执行的命令",

    inject_env_exfiltration: "可疑：命令可能泄露敏感环境变量",
    inject_base64_shell: "可疑：Base64 编码内容被管道传送到 Shell",
    inject_reverse_shell: "可疑：可能的反向 Shell 攻击",
    inject_eval_remote: "可疑：eval 执行远程内容",
    inject_source_remote: "可疑：通过进程替换加载远程内容",
    inject_overwrite_config: "可疑：覆写 Shell 配置文件",
    inject_crontab_modify: "可疑：通过管道修改 crontab",
    inject_download_execute: "可疑：下载-执行链检测到",
    inject_config_file_attack: "可疑：curl 使用配置文件可能读取敏感数据",
};

static EN: T = T {
    cached: "(cached)",
    command_explanation: "Command explanation:",
    diagnosis: "Diagnosis:",
    cancelled: "Cancelled.",
    thinking: "Thinking...",
    analyzing: "Analyzing...",

    danger_warning: "DANGEROUS COMMAND! This could cause data loss or system damage.",
    modify_warning: "This command modifies files or system settings.",

    confirm_dangerous: "⚠ Are you SURE you want to execute this?",
    yes_execute: "Yes, execute (I understand the risks)",
    no_cancel: "No, cancel",
    edit_command: "Edit command",
    execute: "[Y] Execute",
    cancel: "[n] Cancel",
    edit: "[e] Edit",
    edit_prompt: "Edit command",
    exit_code: "Exit code",

    no_piz_record: "No piz execution record found, reading shell history...",
    last_from_history: "Last command from history:",
    last_succeeded: "Last command succeeded. Nothing to fix.",
    failed_command: "Failed command:",

    auto_fix_prompt: "Command failed. Auto-fix?",
    auto_fix_attempting: "Analyzing error and attempting fix...",
    auto_fix_failed: "Auto-fix failed, reason:",

    wizard_title: "⚙ piz configuration wizard",
    select_backend: "Select default LLM backend",

    auto_confirm_prompt: "Auto-execute safe commands without confirmation?",
    extra_backends: "Configure additional backends now?",
    config_saved: "✔ Config saved:",
    config_validated: "✔ Config validated successfully.",
    config_edit_hint: "You can edit it later:",
    config_rerun_hint: "Or re-run: piz config --init",
    config_overwrite: "Config already exists. Overwrite?",
    api_key_prompt: "API key",
    model_prompt: "Model name",
    select_provider: "Select API provider",
    base_url_prompt: "API base URL",
    custom_url_prompt: "Use custom API URL (proxy)?",
    add_openai: "Add OpenAI-compatible backend?",
    add_claude: "Add Claude backend?",
    add_gemini: "Add Gemini backend?",
    add_ollama: "Add Ollama backend?",
    ollama_host: "Ollama host",

    chat_title: "interactive mode",
    chat_hint: "Type your request, or 'exit'/'quit' to leave.",
    bye: "Bye!",

    chat_help_desc: "/help - Show commands  /clear - Clear history  /history - View history",
    chat_clear_desc: "Clear chat history",
    chat_history_desc: "View chat history",
    chat_cleared: "Chat history cleared.",
    chat_unknown_cmd: "Unknown command. Type /help for available commands.",

    select_command: "Select a command to execute",

    inject_env_exfiltration: "Suspicious: command may exfiltrate sensitive environment variables",
    inject_base64_shell: "Suspicious: base64-encoded payload piped to shell",
    inject_reverse_shell: "Suspicious: possible reverse shell attempt",
    inject_eval_remote: "Suspicious: eval with remote content",
    inject_source_remote: "Suspicious: sourcing remote content via process substitution",
    inject_overwrite_config: "Suspicious: overwriting shell configuration",
    inject_crontab_modify: "Suspicious: modifying crontab via pipe",
    inject_download_execute: "Suspicious: download-execute chain detected",
    inject_config_file_attack: "Suspicious: curl with config file may read sensitive data",
};

static JA: T = T {
    cached: "(キャッシュ)",
    command_explanation: "コマンド解説：",
    diagnosis: "診断：",
    cancelled: "キャンセルしました。",
    thinking: "考え中...",
    analyzing: "分析中...",

    danger_warning: "危険なコマンドです！データ損失やシステム障害を引き起こす可能性があります。",
    modify_warning: "このコマンドはファイルやシステム設定を変更します。",

    confirm_dangerous: "⚠ 本当にこの危険なコマンドを実行しますか？",
    yes_execute: "はい、実行する（リスクを理解しています）",
    no_cancel: "いいえ、キャンセル",
    edit_command: "コマンドを編集",
    execute: "[Y] 実行",
    cancel: "[n] キャンセル",
    edit: "[e] 編集",
    edit_prompt: "コマンドを編集",
    exit_code: "終了コード",

    no_piz_record: "piz の実行記録が見つかりません。シェル履歴を読み込んでいます...",
    last_from_history: "履歴のコマンド：",
    last_succeeded: "前回のコマンドは成功しました。修正の必要はありません。",
    failed_command: "失敗したコマンド：",

    auto_fix_prompt: "コマンドが失敗しました。自動修復しますか？",
    auto_fix_attempting: "エラーを分析して修復を試みています...",
    auto_fix_failed: "自動修復に失敗しました。理由：",

    wizard_title: "⚙ piz 設定ウィザード",
    select_backend: "デフォルトの LLM バックエンドを選択",

    auto_confirm_prompt: "安全なコマンドを確認なしで自動実行しますか？",
    extra_backends: "他のバックエンドも設定しますか？",
    config_saved: "✔ 設定を保存しました：",
    config_validated: "✔ 設定の検証に成功しました。",
    config_edit_hint: "後で編集できます：",
    config_rerun_hint: "または再実行：piz config --init",
    config_overwrite: "設定は既に存在します。上書きしますか？",
    api_key_prompt: "APIキー",
    model_prompt: "モデル名",
    select_provider: "APIプロバイダーを選択",
    base_url_prompt: "API ベース URL",
    custom_url_prompt: "カスタム API URL（プロキシ）を使用しますか？",
    add_openai: "OpenAI 互換バックエンドを追加しますか？",
    add_claude: "Claude バックエンドを追加しますか？",
    add_gemini: "Gemini バックエンドを追加しますか？",
    add_ollama: "Ollama バックエンドを追加しますか？",
    ollama_host: "Ollama ホスト",

    chat_title: "インタラクティブモード",
    chat_hint: "リクエストを入力するか、'exit'/'quit' で終了します。",
    bye: "さようなら！",

    chat_help_desc: "/help - コマンド一覧  /clear - 履歴クリア  /history - 履歴表示",
    chat_clear_desc: "チャット履歴をクリア",
    chat_history_desc: "チャット履歴を表示",
    chat_cleared: "チャット履歴をクリアしました。",
    chat_unknown_cmd: "不明なコマンドです。/help で利用可能なコマンドを確認してください。",

    select_command: "実行するコマンドを選択",

    inject_env_exfiltration: "疑わしい：機密環境変数が漏洩する可能性があります",
    inject_base64_shell: "疑わしい：Base64エンコードされたペイロードがシェルにパイプされています",
    inject_reverse_shell: "疑わしい：リバースシェルの可能性があります",
    inject_eval_remote: "疑わしい：リモートコンテンツのeval実行",
    inject_source_remote: "疑わしい：プロセス置換によるリモートコンテンツの読み込み",
    inject_overwrite_config: "疑わしい：シェル設定ファイルの上書き",
    inject_crontab_modify: "疑わしい：パイプによるcrontabの変更",
    inject_download_execute: "疑わしい：ダウンロード-実行チェーンが検出されました",
    inject_config_file_attack: "疑わしい：curlの設定ファイルが機密データを読み取る可能性があります",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_from_code() {
        assert_eq!(Lang::from_code("zh"), Lang::Zh);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("ja"), Lang::Ja);
        assert_eq!(Lang::from_code("EN"), Lang::En);
        assert_eq!(Lang::from_code("unknown"), Lang::Zh); // default
        assert_eq!(Lang::from_code(""), Lang::Zh);
    }

    #[test]
    fn lang_code_roundtrip() {
        assert_eq!(Lang::from_code(Lang::Zh.code()), Lang::Zh);
        assert_eq!(Lang::from_code(Lang::En.code()), Lang::En);
        assert_eq!(Lang::from_code(Lang::Ja.code()), Lang::Ja);
    }

    #[test]
    fn all_langs_have_translations() {
        for lang in [Lang::Zh, Lang::En, Lang::Ja] {
            let tr = t(lang);
            assert!(!tr.cached.is_empty(), "{:?}: cached", lang);
            assert!(
                !tr.command_explanation.is_empty(),
                "{:?}: command_explanation",
                lang
            );
            assert!(!tr.diagnosis.is_empty(), "{:?}: diagnosis", lang);
            assert!(!tr.cancelled.is_empty(), "{:?}: cancelled", lang);
            assert!(!tr.thinking.is_empty(), "{:?}: thinking", lang);
            assert!(!tr.analyzing.is_empty(), "{:?}: analyzing", lang);
            assert!(!tr.danger_warning.is_empty(), "{:?}: danger_warning", lang);
            assert!(!tr.modify_warning.is_empty(), "{:?}: modify_warning", lang);
            assert!(
                !tr.confirm_dangerous.is_empty(),
                "{:?}: confirm_dangerous",
                lang
            );
            assert!(!tr.yes_execute.is_empty(), "{:?}: yes_execute", lang);
            assert!(!tr.no_cancel.is_empty(), "{:?}: no_cancel", lang);
            assert!(!tr.edit_command.is_empty(), "{:?}: edit_command", lang);
            assert!(!tr.execute.is_empty(), "{:?}: execute", lang);
            assert!(!tr.cancel.is_empty(), "{:?}: cancel", lang);
            assert!(!tr.edit.is_empty(), "{:?}: edit", lang);
            assert!(!tr.edit_prompt.is_empty(), "{:?}: edit_prompt", lang);
            assert!(!tr.exit_code.is_empty(), "{:?}: exit_code", lang);
            assert!(!tr.no_piz_record.is_empty(), "{:?}: no_piz_record", lang);
            assert!(
                !tr.last_from_history.is_empty(),
                "{:?}: last_from_history",
                lang
            );
            assert!(!tr.last_succeeded.is_empty(), "{:?}: last_succeeded", lang);
            assert!(!tr.failed_command.is_empty(), "{:?}: failed_command", lang);
            assert!(
                !tr.auto_fix_prompt.is_empty(),
                "{:?}: auto_fix_prompt",
                lang
            );
            assert!(
                !tr.auto_fix_attempting.is_empty(),
                "{:?}: auto_fix_attempting",
                lang
            );
            assert!(
                !tr.auto_fix_failed.is_empty(),
                "{:?}: auto_fix_failed",
                lang
            );
            assert!(!tr.wizard_title.is_empty(), "{:?}: wizard_title", lang);
            assert!(!tr.select_backend.is_empty(), "{:?}: select_backend", lang);
            assert!(
                !tr.auto_confirm_prompt.is_empty(),
                "{:?}: auto_confirm_prompt",
                lang
            );
            assert!(!tr.extra_backends.is_empty(), "{:?}: extra_backends", lang);
            assert!(!tr.config_saved.is_empty(), "{:?}: config_saved", lang);
            assert!(
                !tr.config_validated.is_empty(),
                "{:?}: config_validated",
                lang
            );
            assert!(
                !tr.config_edit_hint.is_empty(),
                "{:?}: config_edit_hint",
                lang
            );
            assert!(
                !tr.config_rerun_hint.is_empty(),
                "{:?}: config_rerun_hint",
                lang
            );
            assert!(
                !tr.config_overwrite.is_empty(),
                "{:?}: config_overwrite",
                lang
            );
            assert!(!tr.api_key_prompt.is_empty(), "{:?}: api_key_prompt", lang);
            assert!(!tr.model_prompt.is_empty(), "{:?}: model_prompt", lang);
            assert!(
                !tr.select_provider.is_empty(),
                "{:?}: select_provider",
                lang
            );
            assert!(
                !tr.base_url_prompt.is_empty(),
                "{:?}: base_url_prompt",
                lang
            );
            assert!(
                !tr.custom_url_prompt.is_empty(),
                "{:?}: custom_url_prompt",
                lang
            );
            assert!(!tr.add_openai.is_empty(), "{:?}: add_openai", lang);
            assert!(!tr.add_claude.is_empty(), "{:?}: add_claude", lang);
            assert!(!tr.add_gemini.is_empty(), "{:?}: add_gemini", lang);
            assert!(!tr.add_ollama.is_empty(), "{:?}: add_ollama", lang);
            assert!(!tr.ollama_host.is_empty(), "{:?}: ollama_host", lang);
            assert!(!tr.chat_title.is_empty(), "{:?}: chat_title", lang);
            assert!(!tr.chat_hint.is_empty(), "{:?}: chat_hint", lang);
            assert!(!tr.bye.is_empty(), "{:?}: bye", lang);
            assert!(!tr.chat_help_desc.is_empty(), "{:?}: chat_help_desc", lang);
            assert!(
                !tr.chat_clear_desc.is_empty(),
                "{:?}: chat_clear_desc",
                lang
            );
            assert!(
                !tr.chat_history_desc.is_empty(),
                "{:?}: chat_history_desc",
                lang
            );
            assert!(!tr.chat_cleared.is_empty(), "{:?}: chat_cleared", lang);
            assert!(
                !tr.chat_unknown_cmd.is_empty(),
                "{:?}: chat_unknown_cmd",
                lang
            );
            assert!(!tr.select_command.is_empty(), "{:?}: select_command", lang);
            assert!(
                !tr.inject_env_exfiltration.is_empty(),
                "{:?}: inject_env_exfiltration",
                lang
            );
            assert!(
                !tr.inject_base64_shell.is_empty(),
                "{:?}: inject_base64_shell",
                lang
            );
            assert!(
                !tr.inject_reverse_shell.is_empty(),
                "{:?}: inject_reverse_shell",
                lang
            );
            assert!(
                !tr.inject_eval_remote.is_empty(),
                "{:?}: inject_eval_remote",
                lang
            );
            assert!(
                !tr.inject_source_remote.is_empty(),
                "{:?}: inject_source_remote",
                lang
            );
            assert!(
                !tr.inject_overwrite_config.is_empty(),
                "{:?}: inject_overwrite_config",
                lang
            );
            assert!(
                !tr.inject_crontab_modify.is_empty(),
                "{:?}: inject_crontab_modify",
                lang
            );
            assert!(
                !tr.inject_download_execute.is_empty(),
                "{:?}: inject_download_execute",
                lang
            );
            assert!(
                !tr.inject_config_file_attack.is_empty(),
                "{:?}: inject_config_file_attack",
                lang
            );
        }
    }
}
