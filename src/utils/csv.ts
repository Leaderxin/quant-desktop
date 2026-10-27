// 自选导入的 CSV 解析。
//
// 三件事都不在组件里做，因为它们各自都有「不报错、只是导错」的失败方式：
// 编码认错 → 整列中文变乱码；引号/换行解析错 → 少导几行；表头认错 → 把表头
// 当成一只股票去搜。放在这里可以逐条钉住。

/** 一行待导入的数据。`line` 用来在失败提示里定位：CSV 是**物理行号**
 * （引号内嵌的换行也占行 —— 否则提示会把用户指到错误的行上）；Excel 没有
 * 物理行的概念，退化为表格行号。 */
export interface CsvImportRow {
  line: number;
  code: string;
  name: string;
}

export interface CsvParseResult {
  rows: CsvImportRow[];
  /** 空行 / 只有逗号的行，静默跳过 */
  skipped: number;
  /** 认出表头时给出实际用到哪几列；null = 没有表头，按前两列解读 */
  header: { code: boolean; name: boolean } | null;
}

const CODE_HEADERS = ['代码', '证券代码', '股票代码', 'code'];
const NAME_HEADERS = ['名称', '证券名称', '股票名称', 'name'];

/** UTF-8 BOM 的字符码。写成数字而不是字面量 —— 它在编辑器里是看不见的。 */
const BOM = 0xfeff;

/**
 * 把文件字节解成文本。
 *
 * 中文 Windows 上 Excel「另存为 CSV」默认是 GBK，而用户从网页/编辑器里
 * 另存的又是 UTF-8。先按 UTF-8 严格解一次（BOM 先剥掉），解不动才退到 GBK ——
 * 反过来先试 GBK 会把 UTF-8 的中文解成乱码而不是报错，属于静默错。
 */
export function decodeCsv(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    return new TextDecoder('utf-8').decode(bytes.subarray(3));
  }
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder('gbk').decode(bytes);
  }
}

/** 带物理行号的一行。`line` 是该行在文件里的物理行号（1 起） */
export interface ParsedCsvRow {
  cells: string[];
  line: number;
}

/**
 * RFC 4180 风格的解析：引号内的逗号与换行属于同一个字段，`""` 表示一个引号。
 * 不依赖任何库 —— 整个应用只在这里用到 CSV，为它引一个解析器不划算。
 *
 * 行号按**物理行**记：引号里嵌的换行属于字段、同时也占一条物理行。文件里
 * 若真有这样的格子，后续行的「第 N 行」必须跟着后移 —— 否则失败提示会把
 * 用户指到错误的行上，而拿着错的行号去改文件，改的永远是别人的行。
 */
export function parseCsvRows(text: string): ParsedCsvRow[] {
  const rows: ParsedCsvRow[] = [];
  let row: string[] = [];
  let field = '';
  let quoted = false;
  let line = 1; // 当前物理行
  let rowStart = 1; // 当前行的起始物理行

  const endField = () => {
    row.push(field);
    field = '';
  };
  const endRow = () => {
    endField();
    rows.push({ cells: row, line: rowStart });
    row = [];
    rowStart = line + 1;
  };

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else {
          quoted = false;
        }
      } else {
        field += ch;
        // 引号内的换行属于字段，但物理行照样前进（CRLF 算一条）
        if (ch === '\n' || (ch === '\r' && text[i + 1] !== '\n')) line++;
      }
    } else if (ch === '"') {
      quoted = true;
    } else if (ch === ',') {
      endField();
    } else if (ch === '\n') {
      endRow();
      line++;
    } else if (ch === '\r') {
      // 孤立的 \r 也当换行（旧版 Excel 会这么存）；CRLF 里的 \r 跳过，
      // 紧随的 \n 负责收行与计数
      if (text[i + 1] !== '\n') {
        endRow();
        line++;
      }
    } else {
      field += ch;
    }
  }
  // 文件末尾没有换行时，最后一行还在缓冲区里
  if (field !== '' || row.length > 0) endRow();

  return rows;
}

/** 只要单元格网格时的便捷包装。 */
export function parseCsv(text: string): string[][] {
  return parseCsvRows(text).map((r) => r.cells);
}

/**
 * 解析成待导入的行。
 *
 * 有表头就按列名找（顺序随意、多一列「备注」也无所谓）；没有表头按「代码,
 * 名称」解读 —— 这是模板的样子，也是最常见的写法。
 */
export function parseWatchlistCsv(text: string): CsvParseResult {
  const body = text.charCodeAt(0) === BOM ? text.slice(1) : text;
  const parsed = parseCsvRows(body);
  return mapWatchlistRows(
    parsed.map((r) => r.cells),
    parsed.map((r) => r.line),
  );
}

/**
 * 把「单元格网格」映射成待导入行。`lines` 与 `table` 逐项对应，是每行的
 * 物理行号 —— CSV 传入；Excel 没有物理行的概念，省略时按表格行号算。
 *
 * CSV 与 Excel 共用这一段：Excel 由后端拆成网格后也走这里，于是「哪列是代码」
 * 「行号怎么算」只有一份实现 —— 两条路径各写一遍，迟早一边认表头一边不认。
 */
export function mapWatchlistRows(table: string[][], lines?: number[]): CsvParseResult {
  if (table.length === 0) return { rows: [], skipped: 0, header: null };

  let codeCol = 0;
  let nameCol = 1;
  let header: CsvParseResult['header'] = null;

  const first = table[0].map((cell) => cell.trim().toLowerCase());
  const codeIdx = first.findIndex((cell) => CODE_HEADERS.includes(cell));
  const nameIdx = first.findIndex((cell) => NAME_HEADERS.includes(cell));
  if (codeIdx >= 0 || nameIdx >= 0) {
    codeCol = codeIdx;
    nameCol = nameIdx;
    header = { code: codeIdx >= 0, name: nameIdx >= 0 };
  }
  const data = header ? table.slice(1) : table;
  const dataOffset = header ? 1 : 0;

  const rows: CsvImportRow[] = [];
  let skipped = 0;
  data.forEach((cells, i) => {
    const code = codeCol >= 0 ? (cells[codeCol] ?? '').trim() : '';
    const name = nameCol >= 0 ? (cells[nameCol] ?? '').trim() : '';
    if (!code && !name) {
      skipped++;
      return;
    }
    const tableIndex = i + dataOffset;
    const line = lines ? lines[tableIndex] : tableIndex + 1;
    rows.push({ line, code, name });
  });

  return { rows, skipped, header };
}
