import { describe, it, expect } from 'vitest';
import { toCsv, csvFilename } from '$lib/export/csv';

describe('toCsv', () => {
  it('joins headers and rows with CRLF', () => {
    const csv = toCsv(['a', 'b'], [['1', '2']]);
    expect(csv).toBe('a,b\r\n1,2\r\n');
  });

  it('leaves plain fields unquoted', () => {
    expect(toCsv(['x'], [['hello world']])).toBe('x\r\nhello world\r\n');
  });

  it('quotes fields containing commas', () => {
    expect(toCsv(['x'], [['a,b']])).toBe('x\r\n"a,b"\r\n');
  });

  it('doubles embedded quotes', () => {
    expect(toCsv(['x'], [['say "hi"']])).toBe('x\r\n"say ""hi"""\r\n');
  });

  it('quotes fields containing newlines', () => {
    expect(toCsv(['x'], [['line1\nline2']])).toBe('x\r\n"line1\nline2"\r\n');
  });

  it('renders null and undefined as empty fields', () => {
    expect(toCsv(['a', 'b', 'c'], [[null, undefined, 'v']])).toBe('a,b,c\r\n,,v\r\n');
  });

  it('renders numbers and booleans as plain text', () => {
    expect(toCsv(['n', 'b'], [[1234.5678, true]])).toBe('n,b\r\n1234.5678,true\r\n');
  });

  it('handles accented text without altering it', () => {
    expect(toCsv(['nom'], [['Épicerie Provigo']])).toBe('nom\r\nÉpicerie Provigo\r\n');
  });

  it('produces a header-only file for zero rows', () => {
    expect(toCsv(['a', 'b'], [])).toBe('a,b\r\n');
  });
});

describe('csvFilename', () => {
  it('uses the provided date', () => {
    expect(csvFilename('balances', '2026-07-05')).toBe('bonscompte-balances-2026-07-05.csv');
  });

  it('defaults to today', () => {
    const today = new Date().toISOString().slice(0, 10);
    expect(csvFilename('transactions')).toBe(`bonscompte-transactions-${today}.csv`);
  });
});
