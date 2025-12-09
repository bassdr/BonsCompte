// frontend/src/lib/utils.ts
export type Split = { member_id: string; weight: number; included?: boolean };

export function computeShares(amount: number, splits: Split[]) {
    const valid = splits.filter(s => s.included !== false && s.weight > 0);
    const total = valid.reduce((a, b) => a + b.weight, 0);

    if (total === 0) {
        return Object.fromEntries(splits.map(s => [s.member_id, 0]));
    }

    const raw: Record<string, number> = {};
    for (const s of splits) {
        if (!valid.includes(s)) raw[s.member_id] = 0;
        else raw[s.member_id] = Math.round((amount * (s.weight / total)) * 100) / 100;
    }

    const sum = Object.values(raw).reduce((a, b) => a + b, 0);
    const diff = Math.round((amount - sum) * 100) / 100;

    if (diff !== 0) {
        const target = valid.reduce((best, cur) =>
            cur.weight > best.weight ? cur : best
        );
        raw[target.member_id] = Math.round((raw[target.member_id] + diff) * 100) / 100;
    }

    return raw;
}
