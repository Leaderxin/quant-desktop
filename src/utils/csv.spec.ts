import { describe, expect, it } from 'vitest';
import { decodeCsv, mapWatchlistRows, parseCsv, parseWatchlistCsv } from './csv';

// 这些用例钉的都是「不会报错、但会导错」的地方：编码认错整列中文变乱码、
// 引号解析错少导几行、表头被当成一只股票去搜。

function utf8(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer as ArrayBuffer;
}

/** 「代码,名称\r\n600519,贵州茅台\r\n」的 GBK 字节（Excel 在中文 Windows 上的默认写法） */
const GBK_CSV = new Uint8Array([
  0xb4, 0xfa, 0xc2, 0xeb, 0x2c, 0xc3, 0xfb, 0xb3, 0xc6, 0x0d, 0x0a,
  0x36, 0x30, 0x30, 0x35, 0x31, 0x39, 0x2c, 0xb9, 0xf3, 0xd6, 0xdd, 0xc3, 0xa9, 0xcc, 0xa8, 0x0d, 0x0a,
]);

describe('decodeCsv', () => {
  it('UTF-8（带不带 BOM 都行）', () => {
    expect(decodeCsv(utf8('代码,名称'))).toBe('代码,名称');
    const bom = new Uint8Array([0xef, 0xbb, 0xbf, ...new TextEncoder().encode('代码,名称')]);
    expect(decodeCsv(bom.buffer as ArrayBuffer)).toBe('代码,名称');
  });

  it('UTF-8 解不动时退到 GBK', () => {
    expect(decodeCsv(GBK_CSV.buffer as ArrayBuffer)).toBe('代码,名称\r\n600519,贵州茅台\r\n');
  });
});

describe('parseCsv', () => {
  it('普通行、CRLF、末尾换行不多出一行', () => {
    expect(parseCsv('a,b\r\nc,d\r\n')).toEqual([['a', 'b'], ['c', 'd']]);
  });

  it('引号里的逗号与换行属于同一个字段', () => {
    expect(parseCsv('"贵州,茅台",600519')).toEqual([['贵州,茅台', '600519']]);
    expect(parseCsv('"第一行\n第二行",x')).toEqual([['第一行\n第二行', 'x']]);
  });

  it('两个引号是一个引号', () => {
    expect(parseCsv('"他说""你好""",x')).toEqual([['他说"你好"', 'x']]);
  });

  it('空字段与行尾空字段都保留', () => {
    expect(parseCsv('600519,\n,贵州茅台')).toEqual([['600519', ''], ['', '贵州茅台']]);
  });
});

describe('parseWatchlistCsv', () => {
  it('中文表头带 BOM，行号从 2 开始', () => {
    const result = parseWatchlistCsv('﻿代码,名称\r\n600519,贵州茅台\r\n000001,平安银行\r\n');
    expect(result.header).toEqual({ code: true, name: true });
    expect(result.rows).toEqual([
      { line: 2, code: '600519', name: '贵州茅台' },
      { line: 3, code: '000001', name: '平安银行' },
    ]);
  });

  it('表头列顺序随意，多出来的列不影响', () => {
    const result = parseWatchlistCsv('备注,名称,代码\r\n核心,贵州茅台,600519');
    expect(result.header).toEqual({ code: true, name: true });
    expect(result.rows).toEqual([{ line: 2, code: '600519', name: '贵州茅台' }]);
  });

  it('英文表头也认', () => {
    const result = parseWatchlistCsv('code,name\n600519,Guizhou Moutai');
    expect(result.rows[0]).toEqual({ line: 2, code: '600519', name: 'Guizhou Moutai' });
  });

  it('没有表头时按「代码,名称」解读', () => {
    const result = parseWatchlistCsv('600519,贵州茅台\n000001,平安银行');
    expect(result.header).toBeNull();
    expect(result.rows.map((r) => r.line)).toEqual([1, 2]);
  });

  it('空行跳过并计数', () => {
    const result = parseWatchlistCsv('代码,名称\r\n\r\n,\r\n600519,贵州茅台\r\n');
    expect(result.skipped).toBe(2);
    expect(result.rows).toEqual([{ line: 4, code: '600519', name: '贵州茅台' }]);
  });

  // 失败提示里的「第 N 行」要能让人在文件里找到那一行：引号内嵌的换行
  // 属于字段、也占一条物理行，后面的行号必须跟着后移 —— 否则用户拿着
  // 错的行号去改文件，改的永远是别人的行。
  it('引号内的换行占物理行，后面的行号跟着走', () => {
    const text = '代码,名称\r\n600519,"贵州\n茅台"\r\n000001,平安银行\r\n';
    const result = parseWatchlistCsv(text);
    expect(result.rows[0]).toMatchObject({ line: 2, code: '600519', name: '贵州\n茅台' });
    expect(result.rows[1]).toMatchObject({ line: 4, code: '000001', name: '平安银行' });
  });

  it('没有表头时同样按物理行记', () => {
    const result = parseWatchlistCsv('600519,贵州茅台\n"多行\n备注",000001\n000001,平安银行');
    expect(result.rows.map((r) => r.line)).toEqual([1, 2, 4]);
  });

  it('只有一列（只有代码或只有名称）', () => {
    const result = parseWatchlistCsv('代码\r\n600519\r\n');
    expect(result.rows).toEqual([{ line: 2, code: '600519', name: '' }]);
  });
});

// Excel 由后端拆成网格后走这里 —— 两条路径共用同一套表头与列映射。
describe('mapWatchlistRows', () => {
  it('与 CSV 的规则一致', () => {
    const result = mapWatchlistRows([
      ['代码', '名称'],
      ['600519', '贵州茅台'],
      ['', ''],
    ]);
    expect(result.header).toEqual({ code: true, name: true });
    expect(result.rows).toEqual([{ line: 2, code: '600519', name: '贵州茅台' }]);
    expect(result.skipped).toBe(1);
  });

  it('Excel 把代码存成数字时原样带出，前导零由后端补', () => {
    const result = mapWatchlistRows([['代码', '名称'], ['1', '平安银行']]);
    expect(result.rows[0]).toEqual({ line: 2, code: '1', name: '平安银行' });
  });
});
