use std::fs::File;
use std::io::Write;

use anyhow::{Context, Result};

use crate::providers::BalanceReport;

pub(crate) fn write_rendered(rendered: &str, output: Option<String>) -> Result<()> {
    match output {
        Some(path) => {
            let mut file = File::create(&path).with_context(|| format!("无法创建文件 {path}"))?;
            writeln!(file, "{rendered}").with_context(|| format!("写入文件 {path} 失败"))?;
        }
        None => println!("{rendered}"),
    }
    Ok(())
}

pub(crate) fn render_balance_text(report: &BalanceReport, usage: f64) -> String {
    let mut output = String::new();
    output.push_str(if report.is_available {
        "状态: 可用\n"
    } else {
        "状态: 不可用\n"
    });
    for info in &report.balance_infos {
        output.push_str(&format!(
            "总余额 {} {}（充值 {} / 赠送 {}）\n",
            info.total_balance, info.currency, info.topped_up_balance, info.granted_balance
        ));
    }
    if let Some(info) = report.balance_infos.first() {
        output.push_str(&format!("今日消耗 {usage:.2} {}\n", info.currency));
    }
    output
}
