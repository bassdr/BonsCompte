// frontend/src/lib/api.ts
const BASE = import.meta.env.VITE_API_BASE ?? "http://localhost:8000";

async function jsonFetch(path: string, opts: RequestInit = {}) {
    const res = await fetch(`${BASE}${path}`, {
        headers: { "Content-Type": "application/json" },
        ...opts
    });

    if (!res.ok) {
        throw new Error(`${res.status} ${res.statusText}: ${await res.text()}`);
    }
    return res.json();
}

export const getMembers = () => jsonFetch("/members");
export const getExpenses = () => jsonFetch("/expenses");

export const postExpense = (payload: unknown) =>
    jsonFetch("/expenses", {
        method: "POST",
        body: JSON.stringify(payload)
    });

export async function uploadReceipt(file: File) {
    const fd = new FormData();
    fd.append("file", file);
    const res = await fetch(`${BASE}/expenses/upload`, {
        method: "POST",
        body: fd
    });

    if (!res.ok) throw new Error(`Upload failed: ${res.statusText}`);

    return res.json(); // expected { url: "/uploads/xyz.png" }
}
