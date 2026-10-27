//! 自选导入：把 CSV / Excel 行批量解析成标的并写入自选。
//!
//! 三个命令分工明确：`import_watchlist` 负责匹配与落库（一次 IPC、一次
//! `watchlist-changed`），`save_import_template` 生成带表头样式的 xlsx 模板，
//! `read_spreadsheet` 把 Excel 拆成单元格网格交给前端。
//!
//! CSV 的解析在前端（编码/引号/表头都在那里，见 src/utils/csv.ts）；后端只把
//! Excel 拆成同样的「网格」，于是表头识别与列映射两条路径共用一套实现。

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use rust_xlsxwriter::{Color, Format, FormatAlign, Workbook};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::datasource::{search, DataSourceManager};
use crate::db::{Database, WatchlistSnapshot};
use crate::domain::StockBrief;
use crate::import::{match_stock, parse_query, search_keyword};

/// 单次导入的行数上限。每行都要打一次搜索接口，500 条在 6 并发下大约半分钟；
/// 再往上就是拿用户的网络和接口的频控，换一个几乎没人用的场景。
pub const MAX_IMPORT_ROWS: usize = 500;

/// 并发搜索的行数。十几行时它把总耗时压掉大半；几百行的大批量靠它把总时长
/// 控制在半分钟内。再高只会互相抢连接，而这些 suggest 接口并不保证对高频请求友好。
const SEARCH_CONCURRENCY: usize = 6;

/// 示例行（用户在模板里覆盖掉即可）
const TEMPLATE_EXAMPLES: [(&str, &str); 2] = [("600519", "贵州茅台"), ("000001", "平安银行")];

/// 表头底色。与应用的强调色同一族（深蓝），白字压在上面足够读。
const HEADER_BG: u32 = 0x1f6feb;

#[derive(Debug, Deserialize)]
pub struct ImportEntry {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ImportRowResult {
    /// 数据行号（1 = CSV 表头之后的第一行），与界面上的「第 N 行」对得上
    pub line: usize,
    /// `added`（已加入）/ `exists`（已在所选分组）/ `failed`（失败）
    pub status: String,
    /// 原始输入，失败时回显给用户看是哪一行
    pub input: String,
    pub code: Option<String>,
    pub name: Option<String>,
    pub market: Option<String>,
    /// 失败原因或补充说明
    pub message: Option<String>,
}

/// 批量导入自选。返回逐行结果，顺序与提交的行一致。
///
/// 先解析、后落库：解析阶段并发打搜索接口（不需要锁库），落库阶段串行，
/// 避免 N 次事务互相竞争那个全局连接锁。
#[tauri::command]
pub async fn import_watchlist(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    manager: State<'_, Arc<DataSourceManager>>,
    entries: Vec<ImportEntry>,
    group_ids: Vec<i64>,
) -> Result<Vec<ImportRowResult>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    if entries.len() > MAX_IMPORT_ROWS {
        return Err(format!(
            "一次最多导入 {} 条（当前 {} 条），请拆分文件后再试",
            MAX_IMPORT_ROWS,
            entries.len()
        ));
    }

    // 导入前拍一张快照：「已在自选」按**导入前**的状态判断，否则同一次导入里
    // 先加进去的那只，再出现一次就会被误报成「已存在」。
    let before = db.get_watchlist_snapshot().map_err(|e| e.to_string())?;
    let membership = membership_map(&before);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(SEARCH_CONCURRENCY));
    let mut tasks = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.into_iter().enumerate() {
        let manager = Arc::clone(&manager);
        let semaphore = Arc::clone(&semaphore);
        tasks.push(tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await;
            let input = [entry.code.trim(), entry.name.trim()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" ");
            let query = parse_query(&entry.code, &entry.name);
            let resolved = if query.code.is_none() && query.name.is_none() {
                Err("缺少代码与名称".to_string())
            } else {
                // 关键字的选择本身有规则（补零行用名称），见 search_keyword
                let keyword = search_keyword(&query);
                let candidates = search::search_cn(&manager, &keyword).await;
                match_stock(&candidates, &query)
            };
            (idx, input, resolved)
        }));
    }

    let mut resolved: Vec<(usize, String, Result<StockBrief, String>)> =
        Vec::with_capacity(tasks.len());
    for task in tasks {
        match task.await {
            Ok(item) => resolved.push(item),
            Err(e) => return Err(format!("导入任务异常: {}", e)),
        }
    }
    resolved.sort_by_key(|(idx, _, _)| *idx);

    let mut results = Vec::with_capacity(resolved.len());
    let mut added_any = false;
    for (idx, input, outcome) in resolved {
        let line = idx + 1;
        match outcome {
            Ok(stock) => {
                let key = (stock.code.clone(), stock.market.clone());
                let already_in = membership.get(&key);
                let in_all_targets = !group_ids.is_empty()
                    && already_in.is_some_and(|set| group_ids.iter().all(|g| set.contains(g)));

                match db.add_watch(&stock.code, &stock.market, &stock.name, &group_ids) {
                    Ok(()) => {
                        if in_all_targets {
                            results.push(ImportRowResult {
                                line,
                                status: "exists".to_string(),
                                input,
                                code: Some(stock.code),
                                name: Some(stock.name),
                                market: Some(stock.market),
                                message: Some("已在所选分组中".to_string()),
                            });
                        } else {
                            added_any = true;
                            results.push(ImportRowResult {
                                line,
                                status: "added".to_string(),
                                input,
                                code: Some(stock.code),
                                name: Some(stock.name),
                                market: Some(stock.market),
                                message: None,
                            });
                        }
                    }
                    Err(e) => results.push(ImportRowResult {
                        line,
                        status: "failed".to_string(),
                        input,
                        code: Some(stock.code),
                        name: Some(stock.name),
                        market: Some(stock.market),
                        message: Some(format!("写入失败: {}", e)),
                    }),
                }
            }
            Err(reason) => results.push(ImportRowResult {
                line,
                status: "failed".to_string(),
                input,
                code: None,
                name: None,
                market: None,
                message: Some(reason),
            }),
        }
    }

    // 只在真的写入了东西时通知：全是「已存在/失败」时重拉一遍自选是白费。
    if added_any {
        let _ = app_handle.emit("watchlist-changed", ());
    }
    Ok(results)
}

/// 把模板写到用户选定的路径 —— 路径来自前端的原生「另存为」对话框。
/// 返回实际写入的路径（没写扩展名时补 `.xlsx`）。
///
/// 模板是 xlsx 而不是 csv，唯一的理由是**表头要有底色**：CSV 是纯文本，
/// Excel 打开时不会带任何样式，填表时很容易把表头当成数据行。顺带的好处是
/// 代码列可以设成文本格式，用户直接敲 `000001` 不会被 Excel 改成 `1`。
#[tauri::command]
pub fn save_import_template(path: String) -> Result<String, String> {
    let path = ensure_xlsx_extension(&path);
    if path.is_empty() {
        return Err("保存路径为空".to_string());
    }
    let path = std::path::PathBuf::from(&path);
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    std::fs::write(&path, build_template()?).map_err(|e| format!("写入模板失败: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

/// 生成模板工作簿（表头样式 + 列宽 + 代码列文本格式 + 两行示例）。
fn build_template() -> Result<Vec<u8>, String> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("自选").map_err(|e| e.to_string())?;

    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(HEADER_BG))
        .set_align(FormatAlign::Center);
    // 代码列整列按文本存：不然 Excel 会把 000001 显示成 1，导回来就丢前导零
    let code_text = Format::new().set_num_format("@");

    sheet
        .write_string_with_format(0, 0, "代码", &header)
        .map_err(|e| e.to_string())?;
    sheet
        .write_string_with_format(0, 1, "名称", &header)
        .map_err(|e| e.to_string())?;
    sheet.set_row_height(0, 22).map_err(|e| e.to_string())?;
    sheet.set_column_width(0, 14).map_err(|e| e.to_string())?;
    sheet.set_column_width(1, 24).map_err(|e| e.to_string())?;
    sheet
        .set_column_format(0, &code_text)
        .map_err(|e| e.to_string())?;

    for (i, (code, name)) in TEMPLATE_EXAMPLES.iter().enumerate() {
        let row = i as u32 + 1;
        sheet
            .write_string_with_format(row, 0, *code, &code_text)
            .map_err(|e| e.to_string())?;
        sheet
            .write_string(row, 1, *name)
            .map_err(|e| e.to_string())?;
    }

    workbook.save_to_buffer().map_err(|e| e.to_string())
}

/// 补 `.xlsx` 扩展名。原生对话框通常按过滤器补，但各平台不保证。
fn ensure_xlsx_extension(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.to_ascii_lowercase().ends_with(".xlsx") {
        trimmed.to_string()
    } else {
        format!("{}.xlsx", trimmed)
    }
}

/// 读 Excel（.xlsx / .xls）的**第一张工作表**，返回原始单元格网格。
///
/// 只拆格子、不认表头：表头识别与列映射与 CSV 共用前端那一份实现，
/// 两处各写一遍迟早分叉成「CSV 能导、Excel 导不了」这种最难解释的差异。
#[tauri::command]
pub fn read_spreadsheet(bytes: Vec<u8>) -> Result<Vec<Vec<String>>, String> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|e| format!("无法识别的表格文件: {}", e))?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| "表格里没有工作表".to_string())?;
    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| format!("读取工作表失败: {}", e))?;
    Ok(range
        .rows()
        .map(|row| row.iter().map(cell_to_text).collect())
        .collect())
}

fn cell_to_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        // 整数值别带 `.0`：代码列被 Excel 存成数字时就是这条路
        Data::Float(f) if f.fract() == 0.0 => format!("{:.0}", f),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::DateTime(dt) => dt.to_string(),
        Data::Error(e) => format!("{:?}", e),
    }
}

/// `(code, market)` → 它当前所属的分组 id 集合。
fn membership_map(snapshot: &WatchlistSnapshot) -> HashMap<(String, String), HashSet<i64>> {
    let mut by_watch_id: HashMap<i64, HashSet<i64>> = HashMap::new();
    for group in &snapshot.groups {
        for watch_id in &group.watch_ids {
            by_watch_id.entry(*watch_id).or_default().insert(group.id);
        }
    }
    snapshot
        .items
        .iter()
        .filter_map(|item| {
            by_watch_id
                .get(&item.id)
                .map(|groups| ((item.code.clone(), item.market.clone()), groups.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{WatchGroup, WatchItem};

    fn item(id: i64, code: &str) -> WatchItem {
        WatchItem {
            id,
            code: code.to_string(),
            market: "CN".to_string(),
            name: code.to_string(),
            added_at: String::new(),
            ticker_enabled: true,
            ticker_order: 0,
        }
    }

    fn group(id: i64, watch_ids: Vec<i64>) -> WatchGroup {
        WatchGroup {
            id,
            name: format!("g{}", id),
            sort_order: 0,
            created_at: String::new(),
            watch_ids,
        }
    }

    #[test]
    fn membership_map_inverts_the_group_membership() {
        let snapshot = WatchlistSnapshot {
            items: vec![item(1, "sh600519"), item(2, "sz000001")],
            groups: vec![group(10, vec![1, 2]), group(11, vec![1])],
            default_group_id: 10,
        };
        let map = membership_map(&snapshot);
        assert_eq!(
            map.get(&("sh600519".to_string(), "CN".to_string())),
            Some(&HashSet::from([10, 11]))
        );
        assert_eq!(
            map.get(&("sz000001".to_string(), "CN".to_string())),
            Some(&HashSet::from([10]))
        );
    }

    #[test]
    fn membership_map_skips_stock_without_any_group() {
        let snapshot = WatchlistSnapshot {
            items: vec![item(1, "sh600519")],
            groups: vec![group(10, vec![])],
            default_group_id: 10,
        };
        assert!(membership_map(&snapshot).is_empty());
    }

    /// 模板必须能被自己的读取端读回来 —— 生成器与解析器分属两个 crate，
    /// 中间任何一处改坏了（工作表名、单元格类型），导出的模板就成了死文件。
    #[test]
    fn template_round_trips_through_the_reader() {
        let bytes = build_template().expect("生成模板");
        let grid = read_spreadsheet(bytes).expect("读回模板");
        assert_eq!(grid[0], vec!["代码", "名称"], "表头是解析端约定的列名");
        assert_eq!(grid[1], vec!["600519", "贵州茅台"]);
        // 代码列是文本格式，前导零原样保留
        assert_eq!(grid[2], vec!["000001", "平安银行"]);
    }

    /// 用户在「另存为」里没打扩展名时补上。
    #[test]
    fn xlsx_extension_is_completed_not_doubled() {
        assert_eq!(ensure_xlsx_extension("C:\\x\\模板"), "C:\\x\\模板.xlsx");
        assert_eq!(ensure_xlsx_extension("  /tmp/模板.XLSX  "), "/tmp/模板.XLSX");
        assert_eq!(ensure_xlsx_extension("模板.xlsx"), "模板.xlsx");
        assert_eq!(ensure_xlsx_extension("   "), "");
    }
}
