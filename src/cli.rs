use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Python(pip) / npm の依存とライセンス情報を収集して可視化するCLI",
    long_about = None
)]
pub struct Cli {
    /// 解析対象ディレクトリ。複数指定可。省略時はカレントディレクトリ。
    #[arg(short, long = "path", value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// JSON出力を書き出すファイルパス。
    #[arg(long = "json-output", value_name = "FILE")]
    pub json_output: Option<PathBuf>,

    /// JSONを標準出力へ出す場合は指定してください。
    #[arg(long = "print-json")]
    pub print_json: bool,

    /// PyPI / npm Registryからライセンス情報を取得してUnknownを補完します。
    #[arg(long = "fetch-licenses")]
    pub fetch_licenses: bool,

    /// テーブルとJSON出力を指定文字列でフィルタします（名前・マネージャ・ライセンス・ソースが対象）。
    #[arg(long = "search", value_name = "QUERY")]
    pub search: Option<String>,

    /// テーブル出力時にSource列を非表示にします。
    #[arg(long = "hide-source")]
    pub hide_source: bool,
}
