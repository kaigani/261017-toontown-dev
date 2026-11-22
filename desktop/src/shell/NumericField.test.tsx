// D-61 items 1-3 (C44A-R1-20, -21): the window's one numeric field and its commit rule.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { declaredRange, parseNumber, type CommitAnswer, type NumericRange } from './numeric';
import { NumericField } from './NumericField';

afterEach(cleanup);

const RANGE: NumericRange = { min: 0, max: 100 };

/** A field over a retained value that a commit replaces unless `refuse` answers a message. */
function Harness(props: {
  initial?: number;
  range?: NumericRange;
  refuse?: string;
  onCommit?: (v: number) => void;
  setter?: (set: (v: number) => void) => void;
}) {
  const [value, setValue] = useState(props.initial ?? 8);
  props.setter?.(setValue);
  return (
    <NumericField
      id="f"
      label="Size"
      value={value}
      range={props.range ?? RANGE}
      onCommit={(v): CommitAnswer => {
        props.onCommit?.(v);
        if (props.refuse) return props.refuse;
        setValue(v);
        return null;
      }}
    />
  );
}

const input = () => screen.getByRole<HTMLInputElement>('textbox', { name: 'Size' });
const problem = () => screen.queryByRole('status')?.textContent ?? null;

async function typeAnd(text: string, key: 'Enter' | 'Escape' | 'blur') {
  fireEvent.change(input(), { target: { value: text } });
  await act(async () => {
    if (key === 'blur') fireEvent.blur(input());
    else fireEvent.keyDown(input(), { key });
    await Promise.resolve();
  });
}

describe('parseNumber', () => {
  it('parses strictly', () => {
    expect(parseNumber('12', RANGE)).toEqual({ ok: true, value: 12 });
    expect(parseNumber(' 2.5 ', RANGE)).toEqual({ ok: true, value: 2.5 });
    expect(parseNumber('.5', RANGE)).toEqual({ ok: true, value: 0.5 });
    expect(parseNumber('1e1', RANGE)).toEqual({ ok: true, value: 10 });
    for (const bad of ['', '   ', 'abc', '12abc', '1,5', '0x10', 'Infinity', 'NaN', '-', '.']) {
      expect(parseNumber(bad, RANGE).ok, bad).toBe(false);
    }
    expect(parseNumber('101', RANGE)).toEqual({ ok: false, problem: 'The value is from 0 to 100.' });
    expect(parseNumber('-1', RANGE)).toEqual({ ok: false, problem: 'The value is from 0 to 100.' });
    expect(parseNumber('2.5', { ...RANGE, integer: true })).toEqual({
      ok: false,
      problem: 'Enter a whole number.',
    });
  });

  it("reads a command's declared range from the generated schema", () => {
    expect(declaredRange('tool.pen.set', 'maxSize')).toEqual({ min: 0, max: 100 });
    expect(declaredRange('tool.text.options.set', 'size')).toMatchObject({ min: 1, max: 512 });
    expect(declaredRange('agent.server.set', 'port')).toEqual({ min: 1024, max: 65535, integer: true });
    expect(declaredRange('no.such.command', 'x')).toEqual({});
  });
});

describe('NumericField', () => {
  it('a valid entry commits on Enter, and the display is the retained value', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} />);
    await typeAnd('12', 'Enter');
    expect(sent.mock.calls).toEqual([[12]]);
    expect(input().value).toBe('12');
    expect(problem()).toBeNull();
  });

  it('focus loss commits; an Enter then focus loss sends once', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} />);
    await typeAnd('30', 'blur');
    expect(sent.mock.calls).toEqual([[30]]);
    fireEvent.change(input(), { target: { value: '40' } });
    fireEvent.keyDown(input(), { key: 'Enter' });
    fireEvent.blur(input());
    await act(async () => {
      await Promise.resolve();
    });
    expect(sent.mock.calls).toEqual([[30], [40]]);
  });

  it('an empty entry sends nothing and returns to the retained value', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} />);
    await typeAnd('', 'Enter');
    expect(sent).not.toHaveBeenCalled();
    expect(input().value).toBe('8');
    expect(problem()).toBe('Enter a number.');
  });

  it('letters are never a number (C44A-R1-21): nothing is sent, the field returns and says why', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} />);
    await typeAnd('abc', 'Enter');
    expect(sent).not.toHaveBeenCalled();
    expect(input().value).toBe('8');
    expect(input().getAttribute('aria-invalid')).toBe('true');
    expect(problem()).toBe('"abc" is not a number.');
    await typeAnd('12x', 'blur');
    expect(sent).not.toHaveBeenCalled();
    expect(input().value).toBe('8');
  });

  it('out of range on either side is refused by the declared range (C44A-R1-20)', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} />);
    await typeAnd('101', 'Enter');
    expect([input().value, problem()]).toEqual(['8', 'The value is from 0 to 100.']);
    await typeAnd('-1', 'blur');
    expect([input().value, problem()]).toEqual(['8', 'The value is from 0 to 100.']);
    expect(sent).not.toHaveBeenCalled();
  });

  it("a command's refusal returns the field to the retained value with the refusal's message", async () => {
    render(<Harness refuse="The drawing is locked." />);
    await typeAnd('50', 'Enter');
    expect(input().value).toBe('8');
    expect(problem()).toBe('The drawing is locked.');
  });

  it('Escape restores without sending, and clears a shown reason', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} />);
    await typeAnd('abc', 'Enter');
    expect(problem()).not.toBeNull();
    await typeAnd('55', 'Escape');
    expect(sent).not.toHaveBeenCalled();
    expect(input().value).toBe('8');
    expect(problem()).toBeNull();
  });

  it('a change made elsewhere while editing shows at once (the draft belongs to its value)', async () => {
    let set: (v: number) => void = () => undefined;
    const sent = vi.fn();
    render(
      <Harness
        onCommit={sent}
        setter={(s) => {
          set = s;
        }}
      />,
    );
    fireEvent.change(input(), { target: { value: '60' } });
    expect(input().value).toBe('60');
    act(() => {
      set(20);
    });
    expect(input().value).toBe('20');
    fireEvent.blur(input());
    await act(async () => {
      await Promise.resolve();
    });
    expect(sent).not.toHaveBeenCalled();
  });

  it('Arrow Up and Down step the draft within the range', () => {
    render(<Harness initial={99.5} range={RANGE} />);
    fireEvent.keyDown(input(), { key: 'ArrowUp' });
    expect(input().value).toBe('100');
    fireEvent.keyDown(input(), { key: 'ArrowUp' });
    expect(input().value).toBe('100');
  });

  it('a whole number is required where the command declares an integer', async () => {
    const sent = vi.fn();
    render(<Harness onCommit={sent} range={{ min: 1, max: 8, integer: true }} initial={2} />);
    await typeAnd('2.5', 'Enter');
    expect(sent).not.toHaveBeenCalled();
    expect([input().value, problem()]).toEqual(['2', 'Enter a whole number.']);
  });
});
